use std::path::Path;

#[derive(Copy, Clone)]
pub struct Context<'a> {
    pub krate: &'a str,
    pub source_dir: &'a Path,
    pub workspace: &'a Path,
}

pub fn trim<S: AsRef<[u8]>>(output: S) -> String {
    let bytes = output.as_ref();
    let mut normalized = String::from_utf8_lossy(bytes).to_string();

    let len = normalized.trim_end().len();
    normalized.truncate(len);

    if !normalized.is_empty() {
        normalized.push('\n');
    }

    normalized
}

/// For a given compiler output, produces the set of saved outputs against which
/// the compiler's output would be considered correct. If the test's saved
/// stderr file is identical to any one of these variations, the test will pass.
///
/// This is a set rather than just one normalized output in order to avoid
/// breaking existing tests when introducing new normalization steps. Someone
/// may have saved stderr snapshots with an older version of trybuild, and those
/// tests need to continue to pass with newer versions of trybuild.
///
/// There is one "preferred" variation which is what we print when the stderr
/// file is absent or not a match.
pub fn diagnostics(output: Vec<u8>, context: Context) -> Variations {
    let mut from_bytes = String::from_utf8_lossy(&output).to_string();
    from_bytes = from_bytes.replace("\r\n", "\n");

    let variations = [
        Basic,
        StripCouldNotCompile,
        StripCouldNotCompile2,
        StripForMoreInformation,
        StripForMoreInformation2,
        DirBackslash,
        TrimEnd,
        RustLib,
    ]
    .iter()
    .map(|normalization| apply(&from_bytes, *normalization, context))
    .collect();

    Variations { variations }
}

pub struct Variations {
    variations: Vec<String>,
}

impl Variations {
    pub fn preferred(&self) -> &str {
        self.variations.last().unwrap()
    }

    pub fn any<F: FnMut(&str) -> bool>(&self, mut f: F) -> bool {
        self.variations.iter().any(|stderr| f(stderr))
    }
}

#[derive(PartialOrd, PartialEq, Copy, Clone)]
enum Normalization {
    Basic,
    StripCouldNotCompile,
    StripCouldNotCompile2,
    StripForMoreInformation,
    StripForMoreInformation2,
    DirBackslash,
    TrimEnd,
    RustLib,
}

use self::Normalization::*;

fn apply(original: &str, normalization: Normalization, context: Context) -> String {
    let mut normalized = String::new();

    for line in original.lines() {
        if let Some(line) = filter(line, normalization, context) {
            normalized += &line;
            if !normalized.ends_with("\n\n") {
                normalized.push('\n');
            }
        }
    }

    trim(normalized)
}

trait SmartReplace {
    fn smart_replace(&self, pat: &str, src: &str) -> String;
}
impl SmartReplace for str {
    fn smart_replace(&self, from: &str, to: &str) -> String {
        if from.is_empty() {
            return self.to_string();
        }
        self.replace(from, to)
    }
}
impl SmartReplace for String {
    fn smart_replace(&self, from: &str, to: &str) -> String {
        <str as SmartReplace>::smart_replace(&self, from, to)
    }
}

fn filter(line: &str, normalization: Normalization, context: Context) -> Option<String> {
    if line.trim_start().starts_with("--> ") {
        if let Some(cut_end) = line.rfind(&['/', '\\'][..]) {
            let cut_start = line.find('>').unwrap() + 2;
            return Some(line[..cut_start].to_owned() + "$DIR/" + &line[cut_end + 1..]);
        }
    }

    if line.trim_start().starts_with("::: ") {
        let mut line = line
            .smart_replace(context.workspace.to_string_lossy().as_ref(), "$WORKSPACE")
            .replace('\\', "/");
        if normalization >= RustLib {
            if let Some(pos) = line.find("/rustlib/src/rust/src/") {
                // ::: $RUST/src/libstd/net/ip.rs:83:1
                line.replace_range(line.find("::: ").unwrap() + 4..pos + 17, "$RUST");
            }
        }
        return Some(line);
    }

    if line.starts_with("error: aborting due to ") {
        return None;
    }

    if line == "To learn more, run the command again with --verbose." {
        return None;
    }

    if normalization >= StripCouldNotCompile {
        if line.starts_with("error: Could not compile `") {
            return None;
        }
    }

    if normalization >= StripCouldNotCompile2 {
        if line.starts_with("error: could not compile `") {
            return None;
        }
    }

    if normalization >= StripForMoreInformation {
        if line.starts_with("For more information about this error, try `rustc --explain") {
            return None;
        }
    }

    if normalization >= StripForMoreInformation2 {
        if line.starts_with("Some errors have detailed explanations:") {
            return None;
        }
        if line.starts_with("For more information about an error, try `rustc --explain") {
            return None;
        }
    }

    let mut line = line.to_owned();

    if normalization >= DirBackslash {
        // https://github.com/dtolnay/trybuild/issues/66
        let source_dir = context.source_dir.to_string_lossy();
        if !source_dir.is_empty() {
            let source_dir_with_backslash = source_dir.into_owned() + "\\";
            line = line.replace(&source_dir_with_backslash, "$DIR/");
        }
    }

    if normalization >= TrimEnd {
        line.truncate(line.trim_end().len());
    }

    line = line
        .smart_replace(context.krate, "$CRATE")
        .smart_replace(context.source_dir.to_string_lossy().as_ref(), "$DIR")
        .smart_replace(context.workspace.to_string_lossy().as_ref(), "$WORKSPACE");

    Some(line)
}
