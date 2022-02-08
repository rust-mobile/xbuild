use crate::Opt;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Flutter {
    path: PathBuf,
}

impl Flutter {
    pub fn from_env() -> Result<Self> {
        let path = dunce::canonicalize(which::which("flutter")?)?
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        Ok(Self { path })
    }

    pub fn engine_version(&self) -> Result<String> {
        let path = self
            .path
            .join("bin")
            .join("internal")
            .join("engine.version");
        if !path.exists() {
            anyhow::bail!("failed to locate engine.version at {}", path.display());
        }
        Ok(std::fs::read_to_string(&path)?.trim().into())
    }

    pub fn assemble(
        &self,
        build_dir: &Path,
        opt: Opt,
        target_platform: &str,
        rules: &[&str],
    ) -> Result<()> {
        let status = Command::new("flutter")
            .arg("assemble")
            .arg("--no-version-check")
            .arg("--depfile")
            .arg(build_dir.join("flutter_build.d"))
            .arg("--output")
            .arg(build_dir.join("assets"))
            .arg(format!("-dTargetPlatform={}", target_platform))
            .arg(format!("-dBuildMode={}", opt))
            .arg("-dTrackWidgetCreation=true")
            .args(rules)
            .status()?;
        if !status.success() {
            anyhow::bail!("flutter assemble exited with {:?}", status);
        }
        Ok(())
    }
}
