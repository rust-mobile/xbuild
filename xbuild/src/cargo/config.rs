use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub build: Option<Build>,
}

impl Config {
    pub fn parse_from_toml(path: impl AsRef<Path>) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&contents)?)
    }

    /// Search for and open `.cargo/config.toml` in any parent of the workspace root path.
    pub fn find_cargo_config_for_workspace(workspace: impl AsRef<Path>) -> Result<Option<Self>> {
        let workspace = workspace.as_ref();
        let workspace = dunce::canonicalize(workspace)?;
        workspace
            .ancestors()
            .map(|dir| dir.join(".cargo/config.toml"))
            .find(|p| p.is_file())
            .map(Config::parse_from_toml)
            .transpose()
    }
}

#[derive(Debug, Deserialize)]
pub struct Build {
    #[serde(rename = "target-dir")]
    pub target_dir: Option<String>,
}
