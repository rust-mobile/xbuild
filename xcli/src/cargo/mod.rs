use crate::sdk::android::Ndk;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;
use xapk::Target;

pub mod readelf;

pub fn cargo_ndk(ndk: &Ndk, target: Target, sdk_version: u32) -> Result<Command> {
    let triple = target.rust_triple();
    let mut cargo = Command::new("cargo");

    let (clang, clang_pp) = ndk.clang(target, sdk_version)?;
    cargo.env(format!("CC_{}", triple), &clang);
    cargo.env(format!("CXX_{}", triple), &clang_pp);
    cargo.env(cargo_env_target_cfg("LINKER", triple), &clang);

    let ar = ndk.toolchain_bin("ar", target)?;
    cargo.env(format!("AR_{}", triple), &ar);
    cargo.env(cargo_env_target_cfg("AR", triple), &ar);

    Ok(cargo)
}

fn cargo_env_target_cfg(tool: &str, target: &str) -> String {
    let utarget = target.replace("-", "_");
    let env = format!("CARGO_TARGET_{}_{}", &utarget, tool);
    env.to_uppercase()
}

pub fn get_libs_search_paths(
    target_dir: &Path,
    target_triple: &str,
    target_profile: &Path,
) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    let deps_dir = target_dir
        .join(target_triple)
        .join(target_profile)
        .join("build");

    for dep_dir in deps_dir.read_dir()? {
        let output_file = dep_dir?.path().join("output");
        if output_file.is_file() {
            use std::{
                fs::File,
                io::{BufRead, BufReader},
            };
            for line in BufReader::new(File::open(output_file)?).lines() {
                let line = line?;
                if let Some(link_search) = line.strip_prefix("cargo:rustc-link-search=") {
                    let mut pie = link_search.split('=');
                    let (kind, path) = match (pie.next(), pie.next()) {
                        (Some(kind), Some(path)) => (kind, path),
                        (Some(path), None) => ("all", path),
                        _ => unreachable!(),
                    };
                    match kind {
                        // FIXME: which kinds of search path we interested in
                        "dependency" | "native" | "all" => paths.push(path.into()),
                        _ => (),
                    };
                }
            }
        }
    }
    Ok(paths)
}
