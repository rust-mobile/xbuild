use crate::Opt;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Platform {
    Android,
    Darwin,
    Linux,
    Ios,
    Windows,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Android => write!(f, "android"),
            Self::Darwin => write!(f, "darwin"),
            Self::Linux => write!(f, "linux"),
            Self::Ios => write!(f, "ios"),
            Self::Windows => write!(f, "windows"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arch {
    Arm,
    Arm64,
    X64,
    X86,
}

impl std::fmt::Display for Arch {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Arm => write!(f, "arm"),
            Self::Arm64 => write!(f, "arm64"),
            Self::X64 => write!(f, "x64"),
            Self::X86 => write!(f, "x86"),
        }
    }
}

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

    pub fn engine_dir(&self, platform: Platform, arch: Arch, opt: Opt) -> Result<PathBuf> {
        let name = if opt == Opt::Debug {
            format!("{}-{}", platform, arch)
        } else {
            format!("{}-{}-{}", platform, arch, opt)
        };
        let path = self
            .path
            .join("bin")
            .join("cache")
            .join("artifacts")
            .join("engine")
            .join(name);
        if !path.exists() {
            anyhow::bail!("engine not found for {} {} {}", platform, arch, opt);
        }
        Ok(path)
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
            .arg("--suppress-analytics")
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
