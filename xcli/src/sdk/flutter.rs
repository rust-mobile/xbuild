use crate::Opt;
use anyhow::Result;
use std::path::PathBuf;
use xapk::Target;

pub struct Flutter {
    path: PathBuf,
}

impl Flutter {
    pub fn from_env() -> Result<Self> {
        let path = dunce::canonicalize(which::which("flutter")?)?
            .parent()
            .unwrap()
            .join("cache")
            .join("artifacts")
            .join("engine");
        if !path.exists() {
            anyhow::bail!("failed to locate flutter engine at {}", path.display());
        }
        Ok(Self { path })
    }

    pub fn flutter_jar(&self, target: Target, opt: Opt) -> Result<PathBuf> {
        let path = self
            .path
            .join(format!(
                "android-{}{}",
                target.flutter_arch(),
                opt.flutter_suffix()
            ))
            .join("flutter.jar");
        if !path.exists() {
            anyhow::bail!("failed to locate flutter.jar at {}", path.display());
        }
        Ok(path)
    }
}

impl Opt {
    pub fn flutter_suffix(self) -> &'static str {
        match self {
            Self::Debug => "",
            Self::Release => "-release",
        }
    }
}
