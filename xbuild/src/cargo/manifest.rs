use anyhow::{Context, Result};
use serde::Deserialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use super::utils;

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Inheritable<T> {
    Value(T),
    Inherited { workspace: bool },
}

#[derive(Clone, Debug, Deserialize)]
pub struct Manifest {
    pub workspace: Option<Workspace>,
    pub package: Option<Package>,
    #[serde(default)]
    pub dependencies: HashMap<String, Dependency>,
}

impl Manifest {
    pub fn parse_from_toml(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&contents)?)
    }

    /// Returns a mapping from manifest directory to manifest path and loaded manifest
    pub fn members(&self, workspace_root: &Path) -> Result<HashMap<PathBuf, (PathBuf, Manifest)>> {
        let workspace = self
            .workspace
            .as_ref()
            .context("The provided Cargo.toml does not contain a `[workspace]`")?;
        let workspace_root = utils::canonicalize(workspace_root)?;

        let mut member_dirs = vec![];

        // resolve members and exclude globs
        let exclude = workspace
            .exclude
            .iter()
            .map(|g| glob::Pattern::new(workspace_root.join(g).to_str().unwrap()))
            .collect::<Result<Vec<_>, _>>()?;
        for member in &workspace.members {
            for manifest_dir in glob::glob(workspace_root.join(member).to_str().unwrap())? {
                let manifest_dir = manifest_dir?;
                if !manifest_dir.is_dir() || exclude.iter().any(|g| g.matches_path(&manifest_dir)) {
                    continue;
                }
                member_dirs.push(manifest_dir);
            }
        }

        // include all local path dependencies
        if self.package.is_some() {
            for dep in self.dependencies.values() {
                if let Dependency::Table { path: Some(path) } = dep {
                    let manifest_dir = workspace_root.join(path);
                    if manifest_dir.starts_with(&workspace_root) {
                        member_dirs.push(manifest_dir);
                    }
                }
            }
        }

        // Check all member packages inside the workspace
        let mut all_members = HashMap::new();

        for manifest_dir in member_dirs {
            if all_members.contains_key(&manifest_dir) {
                continue;
            }

            let manifest_path = manifest_dir.join("Cargo.toml");
            let manifest = Manifest::parse_from_toml(&manifest_path).with_context(|| {
                format!(
                    "Failed to load manifest for workspace member `{}`",
                    manifest_dir.display()
                )
            })?;

            // Workspace members cannot themselves be/contain a new workspace
            anyhow::ensure!(
                manifest.workspace.is_none(),
                "Did not expect a `[workspace]` at `{}`",
                manifest_path.display(),
            );

            // And because they cannot contain a [workspace], they may not be a virtual manifest
            // and must hence contain [package]
            anyhow::ensure!(
                    manifest.package.is_some(),
                    "Failed to parse manifest at `{}`: virtual manifests must be configured with `[workspace]`",
                    manifest_path.display(),
                );

            all_members.insert(manifest_dir, (manifest_path, manifest));
        }

        Ok(all_members)
    }

    /// Returns `self` if it contains `[package]` but not `[workspace]`, (i.e. it cannot be
    /// a workspace nor a virtual manifest), and describes a package named `name` if not [`None`].
    pub fn map_nonvirtual_package(
        self,
        manifest_path: PathBuf,
        name: Option<&str>,
    ) -> Result<(PathBuf, Self)> {
        anyhow::ensure!(
            self.workspace.is_none(),
            "Did not expect a `[workspace]` at `{}`",
            manifest_path.display(),
        );

        if let Some(package) = &self.package {
            if let Some(name) = name {
                if package.name == name {
                    Ok((manifest_path, self))
                } else {
                    Err(anyhow::anyhow!(
                        "package `{}` not found in workspace `{}`",
                        manifest_path.display(),
                        name,
                    ))
                }
            } else {
                Ok((manifest_path, self))
            }
        } else {
            Err(anyhow::anyhow!(
                "Failed to parse manifest at `{}`: virtual manifests must be configured with `[workspace]`",
                manifest_path.display(),
            ))
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Workspace {
    #[serde(default)]
    pub default_members: Vec<String>,
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default)]
    pub exclude: Vec<String>,

    pub package: Option<WorkspacePackage>,
}

/// Almost the same as [`Package`], except that this must provide
/// root values instead of possibly inheritable values
#[derive(Clone, Debug, Deserialize)]
pub struct WorkspacePackage {
    pub version: Option<String>,
    pub description: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Dependency {
    Table { path: Option<PathBuf> },
    Version(String),
}

#[derive(Clone, Debug, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: Inheritable<String>,
    pub description: Option<Inheritable<String>>,
}
