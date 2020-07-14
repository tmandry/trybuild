use serde::Deserialize;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::error::{Error, Result};
use crate::manifest::Name;
use crate::run::{CargoProject, StrategyResult};
use crate::rustflags;

#[derive(Deserialize)]
pub struct Metadata {
    pub target_directory: PathBuf,
    pub workspace_root: PathBuf,
}

fn raw_cargo() -> Command {
    Command::new(option_env!("CARGO").unwrap_or("cargo"))
}

fn cargo(project: &CargoProject) -> Command {
    let mut cmd = raw_cargo();
    cmd.current_dir(&project.dir);
    cmd.env(
        "CARGO_TARGET_DIR",
        path!(project.target_dir / "tests" / "target"),
    );
    cmd.arg("--offline");
    rustflags::set_env(&mut cmd);
    cmd
}

pub fn build_dependencies(project: &CargoProject) -> Result<()> {
    let _ = cargo(project).arg("generate-lockfile").status();

    let status = cargo(project)
        .arg(if project.has_pass { "build" } else { "check" })
        .arg("--bin")
        .arg(&project.name)
        .args(features(project))
        .status()
        .map_err(Error::Cargo)?;

    if status.success() {
        Ok(())
    } else {
        Err(Error::CargoFail)
    }
}

pub fn build_test(project: &CargoProject, name: &Name) -> StrategyResult<Command> {
    let _ = cargo(project)
        .arg("clean")
        .arg("--package")
        .arg(&project.name)
        .arg("--color=never")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    let mut cmd = cargo(project);
    cmd.arg(if project.has_pass { "build" } else { "check" })
        .arg("--bin")
        .arg(name)
        .args(features(project))
        .arg("--quiet")
        .arg("--color=never");
    Ok(cmd)
}

pub fn run_test(project: &CargoProject, name: &Name) -> StrategyResult<Command> {
    let mut cmd = cargo(project);
    cmd.arg("run")
        .arg("--bin")
        .arg(name)
        .args(features(project))
        .arg("--quiet")
        .arg("--color=never");
    Ok(cmd)
}

pub fn metadata() -> Result<Metadata> {
    let output = raw_cargo()
        .arg("metadata")
        .arg("--format-version=1")
        .output()
        .map_err(Error::Cargo)?;

    serde_json::from_slice(&output.stdout).map_err(|err| {
        print!("{}", String::from_utf8_lossy(&output.stderr));
        Error::Metadata(err)
    })
}

fn features(project: &CargoProject) -> Vec<String> {
    match &project.features {
        Some(features) => vec![
            "--no-default-features".to_owned(),
            "--features".to_owned(),
            features.join(","),
        ],
        None => vec![],
    }
}
