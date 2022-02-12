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
    triple: Option<&'static str>,
    c_flags: String,
    cxx_flags: String,
    rust_flags: String,
}

impl Cargo {
    pub fn new(target: CompileTarget) -> Result<Self> {
        let triple = if target.platform() != Platform::host()? || target.arch() != Arch::host()? {
            Some(target.rust_triple()?)
        } else {
            None
        };
        let mut cmd = Command::new("cargo");
        cmd.arg("build");
        if target.opt() == Opt::Release {
            cmd.arg("--release");
        }
        if let Some(triple) = triple.as_ref() {
            cmd.arg("--target").arg(triple);
        }
        Ok(Self {
            cmd,
            target,
            triple,
            c_flags: "".into(),
            cxx_flags: "".into(),
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

    pub fn use_xwin(&mut self, path: &Path) -> Result<()> {
        self.cfg_tool(Tool::Cc, "clang");
        self.cfg_tool(Tool::Cxx, "clang++");
        self.cfg_tool(Tool::Ar, "llvm-lib");
        self.cfg_tool(Tool::Linker, "rust-lld");
        self.add_lib_dir(&path.join("crt").join("lib").join("x86_64"))?;
        self.add_lib_dir(&path.join("sdk").join("lib").join("um").join("x86_64"))?;
        self.add_lib_dir(&path.join("sdk").join("lib").join("ucrt").join("x86_64"))?;
        self.add_target_feature("+crt-static");
        self.use_ld("lld-link");
        self.add_include_dir(&path.join("crt").join("include"))?;
        self.add_include_dir(&path.join("sdk").join("include").join("um"))?;
        self.add_include_dir(&path.join("sdk").join("include").join("ucrt"))?;
        self.add_include_dir(&path.join("sdk").join("include").join("shared"))?;
        Ok(())
    }

    pub fn cfg_tool<P: AsRef<Path>>(&mut self, tool: Tool, path: P) {
        match tool {
            Tool::Cc | Tool::Cxx | Tool::Ar => {
                self.cc_triple_env(&tool.to_string(), path.as_ref().to_str().unwrap());
            }
            Tool::Linker => {
                self.cargo_target_env("LINKER", path.as_ref().to_str().unwrap());
            }
        }
    }

    /// Configures a cargo target specific environment variable.
    fn cargo_target_env(&mut self, name: &str, value: &str) {
        if let Some(triple) = self.triple {
            let utarget = triple.replace("-", "_");
            let env = format!("CARGO_TARGET_{}_{}", &utarget, name);
            self.cmd.env(env.to_uppercase(), value);
        } else {
            self.cmd.env(name, value);
        }
    }

    /// Configures an environment variable for the `cc` crate.
    fn cc_triple_env(&mut self, name: &str, value: &str) {
        if let Some(triple) = self.triple {
            self.cmd.env(format!("{}_{}", name, triple), value);
        } else {
            self.cmd.env(name, value);
        }
    }

    pub fn add_lib_dir(&mut self, path: &Path) -> Result<()> {
        let path = path.canonicalize()?;
        self.rust_flags
            .push_str(&format!("-Lnative={} ", path.display()));
        Ok(())
    }

    pub fn link_lib(&mut self, name: &str) {
        self.rust_flags.push_str(&format!("-l{}", name));
    }

    pub fn add_target_feature(&mut self, target_feature: &str) {
        self.rust_flags
            .push_str(&format!("-Ctarget-feature={} ", target_feature));
    }

    pub fn add_include_dir(&mut self, path: &Path) -> Result<()> {
        let path = path.canonicalize()?;
        self.c_flags.push_str(&format!("-I{} ", path.display()));
        self.cxx_flags.push_str(&format!("-I{} ", path.display()));
        Ok(())
    }

    pub fn use_ld(&mut self, name: &str) {
        self.c_flags.push_str(&format!("-fuse-ld={} ", name));
        self.cxx_flags.push_str(&format!("-fuse-ld={} ", name));
    }

    pub fn build(&mut self) -> Result<()> {
        self.cargo_target_env("RUSTFLAGS", &self.rust_flags.clone());
        self.cc_triple_env("CFLAGS", &self.c_flags.clone());
        self.cc_triple_env("CXXFLAGS", &self.cxx_flags.clone());
        if !self.cmd.status()?.success() {
            anyhow::bail!("cargo build failed");
        }
        Ok(())
    }

    pub fn search_paths(&self, target_dir: &Path) -> Result<Vec<PathBuf>> {
        let mut paths = vec![];
        let opt = self.target.opt().to_string();
        let target_dir = if let Some(triple) = self.triple.as_ref() {
            target_dir.join(triple).join(&opt)
        } else {
            target_dir.join(&opt)
        };
        let deps_dir = target_dir.join("build");

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
