use crate::android::AndroidNdk;
use crate::{Arch, CompileTarget, Opt, Platform};
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

pub mod readelf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tool {
    Cc,
    Cxx,
    Linker,
    Ar,
}

impl std::fmt::Display for Tool {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Cc => write!(f, "CC"),
            Self::Cxx => write!(f, "CXX"),
            Self::Linker => write!(f, "LINKER"),
            Self::Ar => write!(f, "AR"),
        }
    }
}

pub struct Cargo {
    cmd: Command,
    target: CompileTarget,
    triple: &'static str,
    rust_flags: String,
}

impl Cargo {
    pub fn new(target: CompileTarget) -> Result<Self> {
        let triple = target.rust_triple()?;
        let mut cmd = Command::new("cargo");
        cmd.arg("build");
        if target.opt() == Opt::Release {
            cmd.arg("--release");
        }
        if target.platform() != Platform::host()? || target.arch() != Arch::host()? {
            cmd.arg("--target").arg(&triple);
        }
        Ok(Self {
            cmd,
            target,
            triple,
            rust_flags: "".into(),
        })
    }

    pub fn use_ndk_tools(&mut self, ndk: &AndroidNdk, sdk_version: u32) -> Result<()> {
        let android_abi = self.target.android_abi()?;
        let (clang, clang_pp) = ndk.clang(android_abi, sdk_version)?;
        self.cfg_tool(Tool::Cc, &clang);
        self.cfg_tool(Tool::Cxx, &clang_pp);
        self.cfg_tool(Tool::Linker, &clang);
        self.cfg_tool(Tool::Ar, &ndk.toolchain_bin("ar", android_abi)?);
        Ok(())
    }

    pub fn cfg_tool(&mut self, tool: Tool, path: &Path) {
        match tool {
            Tool::Cc | Tool::Cxx => {
                self.cmd.env(format!("{}_{}", tool, self.triple), path);
            }
            Tool::Linker | Tool::Ar => {
                let utarget = self.triple.replace("-", "_").to_uppercase();
                let env = format!("CARGO_TARGET_{}_{}", &utarget, tool);
                self.cmd.env(env, path);
            }
        }
    }

    pub fn add_lib_dir(&mut self, path: &Path) {
        self.rust_flags
            .push_str(&format!("-Clink-arg=-L{} ", path.display()));
    }

    pub fn build(&mut self) -> Result<()> {
        self.cmd.env("RUSTFLAGS", &self.rust_flags);
        if !self.cmd.status()?.success() {
            anyhow::bail!("cargo build failed");
        }
        Ok(())
    }

    pub fn search_paths(&self, target_dir: &Path) -> Result<Vec<PathBuf>> {
        let mut paths = vec![];
        let deps_dir = target_dir
            .join(&self.triple)
            .join(&self.target.opt().to_string())
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
}
