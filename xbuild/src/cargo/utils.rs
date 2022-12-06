use super::config::Config;
use super::manifest::Manifest;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub fn list_rust_files(dir: &Path) -> Result<Vec<String>> {
    let mut files = Vec::new();
    if dir.exists() && dir.is_dir() {
        let entries = std::fs::read_dir(dir)?;
        for entry in entries {
            let path = entry?.path();
            if path.is_file() && path.extension() == Some(OsStr::new("rs")) {
                let name = path.file_stem().unwrap().to_str().unwrap();
                files.push(name.to_string());
            }
        }
    }
    Ok(files)
}

pub fn canonicalize(mut path: &Path) -> Result<PathBuf> {
    if path == Path::new("") {
        path = Path::new(".");
    }
    dunce::canonicalize(path)
        .with_context(|| format!("Failed to canonicalize `{}`", path.display()))
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PackageSelector<'a> {
    ByName(&'a str),
    ByPath(&'a Path),
}

/// Tries to find a package by the given `name` in the [workspace root] or member
/// of the given [workspace] [`Manifest`].
///
/// When a workspace is not detected, call [`find_package_manifest()`] instead.
///
/// [workspace root]: https://doc.rust-lang.org/cargo/reference/workspaces.html#root-package
/// [workspace]: https://doc.rust-lang.org/cargo/reference/workspaces.html#workspaces
pub fn find_package_manifest_in_workspace(
    workspace_manifest_path: &Path,
    workspace_manifest: &Manifest,
    selector: PackageSelector<'_>,
) -> Result<(PathBuf, Manifest)> {
    let workspace = workspace_manifest
        .workspace
        .as_ref()
        .context("The provided Cargo.toml does not contain a `[workspace]`")?;
    let workspace_root = workspace_manifest_path.parent().unwrap();
    let workspace_root = canonicalize(workspace_root)?;

    // Check all member packages inside the workspace
    let mut all_members = HashMap::new();

    for member in &workspace.members {
        for manifest_dir in glob::glob(workspace_root.join(member).to_str().unwrap())? {
            let manifest_dir = manifest_dir?;
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

            all_members.insert(manifest_dir, (manifest_path, manifest));
        }
    }

    match selector {
        PackageSelector::ByName(name) => {
            // Check if the workspace manifest also contains a [package]
            if let Some(package) = &workspace_manifest.package {
                if package.name == name {
                    return Ok((
                        workspace_manifest_path.to_owned(),
                        workspace_manifest.clone(),
                    ));
                }
            }

            // Check all member packages inside the workspace
            for (_manifest_dir, (manifest_path, manifest)) in all_members {
                if let Some(package) = &manifest.package {
                    if package.name == name {
                        return Ok((manifest_path, manifest));
                    }
                } else {
                    anyhow::bail!(
                        "Failed to parse manifest at `{}`: virtual manifests must be configured with `[workspace]`",
                        manifest_path.display(),
                    );
                }
            }

            Err(anyhow::anyhow!(
                "package `{}` not found in workspace `{}`",
                workspace_manifest_path.display(),
                name,
            ))
        }
        PackageSelector::ByPath(path) => {
            let path = canonicalize(path)?;

            // Find the closest member based on the given path
            Ok(path
                .ancestors()
                // Move manifest out of the HashMap
                .find_map(|dir| all_members.remove(dir))
                .unwrap_or_else(|| {
                    (
                        workspace_manifest_path.to_owned(),
                        workspace_manifest.clone(),
                    )
                }))
        }
    }
}

/// Recursively walk up the directories until finding a `Cargo.toml`
///
/// When a workspace has been detected, use [`find_package_manifest_in_workspace()`] to find packages
/// instead (that are members of the given workspace).
pub fn find_package_manifest(path: &Path, name: Option<&str>) -> Result<(PathBuf, Manifest)> {
    let path = canonicalize(path)?;
    let manifest_path = path
        .ancestors()
        .map(|dir| dir.join("Cargo.toml"))
        .find(|manifest| manifest.exists())
        .context("Didn't find Cargo.toml.")?;

    let manifest = Manifest::parse_from_toml(&manifest_path)?;

    // This function shouldn't be called when a workspace exists.
    anyhow::ensure!(
        manifest.workspace.is_none(),
        "Did not expect a `[workspace]` at `{}`",
        manifest_path.display(),
    );

    if let Some(package) = &manifest.package {
        if let Some(name) = name {
            if package.name == name {
                Ok((manifest_path, manifest))
            } else {
                Err(anyhow::anyhow!(
                    "package `{}` not found in workspace `{}`",
                    manifest_path.display(),
                    name,
                ))
            }
        } else {
            Ok((manifest_path, manifest))
        }
    } else {
        Err(anyhow::anyhow!(
            "Failed to parse manifest at `{}`: virtual manifests must be configured with `[workspace]`",
            manifest_path.display(),
        ))
    }
}

/// Find the first `Cargo.toml` that contains a `[workspace]`
pub fn find_workspace(potential_root: &Path) -> Result<Option<(PathBuf, Manifest)>> {
    for manifest_path in potential_root
        .ancestors()
        .map(|dir| dir.join("Cargo.toml"))
        .filter(|manifest| manifest.exists())
    {
        let manifest = Manifest::parse_from_toml(&manifest_path)?;
        if manifest.workspace.is_some() {
            return Ok(Some((manifest_path, manifest)));
        }
    }
    Ok(None)
}

/// Returns the [`target-dir`] configured in `.cargo/config.toml` or `"target"` if not set.
///
/// [`target-dir`]: https://doc.rust-lang.org/cargo/reference/config.html#buildtarget-dir
pub fn get_target_dir_name(config: Option<&Config>) -> Result<String> {
    if let Some(config) = config {
        if let Some(build) = config.build.as_ref() {
            if let Some(target_dir) = &build.target_dir {
                return Ok(target_dir.clone());
            }
        }
    }
    Ok("target".to_string())
}
