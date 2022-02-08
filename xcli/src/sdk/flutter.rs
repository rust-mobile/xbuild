use anyhow::Result;
use std::path::PathBuf;

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
}
