use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Clone, Debug, Deserialize)]
pub struct Manifest {
    pub workspace: Option<Workspace>,
    pub package: Option<Package>,
}

impl Manifest {
    pub fn parse_from_toml(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&contents)?)
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Workspace {
    #[serde(default)]
    pub default_members: Vec<String>,
    #[serde(default)]
    pub members: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
}
