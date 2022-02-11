use crate::{Arch, CompileTarget, Opt, Platform};
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Rule {
    CopyFlutterBundle,
    CopyFlutterAotBundle,
}

impl std::fmt::Display for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::CopyFlutterBundle => write!(f, "copy_flutter_bundle"),
            Self::CopyFlutterAotBundle => write!(f, "copy_flutter_aot_bundle"),
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

    pub fn engine_dir(&self, target: CompileTarget) -> Result<PathBuf> {
        let platform = if target.platform() == Platform::Macos {
            "darwin".to_string()
        } else {
            target.platform().to_string()
        };
        let name = if target.opt() == Opt::Debug {
            format!("{}-{}", platform, target.arch())
        } else {
            format!("{}-{}-{}", platform, target.arch(), target.opt())
        };
        let path = self
            .path
            .join("bin")
            .join("cache")
            .join("artifacts")
            .join("engine")
            .join(name);
        if !path.exists() {
            // TODO: precache when engine version changes
            let status = Command::new("flutter")
                .arg("precache")
                .arg("-v")
                .arg("--suppress-analytics")
                .arg(format!("--{}", target.platform()))
                .status()?;
            if !status.success() {
                anyhow::bail!("flutter precache exited with code {}", status);
            }
        }
        Ok(path)
    }

    pub fn assemble(
        &self,
        target_file: &Path,
        flutter_assets: &Path,
        depfile: &Path,
        target: CompileTarget,
        rule: Rule,
    ) -> Result<()> {
        let target_platform = match (target.platform(), target.arch()) {
            (Platform::Android, _) => "android",
            (Platform::Ios, _) => "ios",
            (Platform::Linux, Arch::Arm64) => "linux-arm64",
            (Platform::Linux, Arch::X64) => "linux-x64",
            (Platform::Macos, _) => "darwin",
            (Platform::Windows, Arch::X64) => "windows-x64",
            _ => anyhow::bail!(
                "unsupported platform arch combination {} {}",
                target.platform(),
                target.arch(),
            ),
        };
        let status = Command::new("flutter")
            .arg("assemble")
            .arg("--no-version-check")
            .arg("--suppress-analytics")
            .arg("--depfile")
            .arg(depfile)
            .arg("--output")
            .arg(flutter_assets)
            .arg(format!("-dTargetPlatform={}", target_platform))
            .arg(format!("-dBuildMode={}", target.opt()))
            .arg("-dTrackWidgetCreation=true")
            .arg(format!("-dTargetFile={}", target_file.display()))
            .arg(rule.to_string())
            .status()?;
        if !status.success() {
            anyhow::bail!("flutter assemble exited with {:?}", status);
        }
        Ok(())
    }
}
