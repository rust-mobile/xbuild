use anyhow::Result;
use std::path::Path;

pub mod config;

pub use crate::config::Config;

#[derive(Clone, Copy, Debug)]
pub enum Format {
    App,
    Apk,
    Appimage,
    Dmg,
    Ipa,
    Msix,
}

impl Format {
    pub fn from_path(path: &Path) -> Result<Self> {
        let ext = path
            .extension()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default()
            .to_lowercase();
        Ok(match ext.as_str() {
            "apk" => Self::Apk,
            "appimage" => Self::Appimage,
            "msix" => Self::Msix,
            ext => anyhow::bail!("unrecognized extension {}", ext),
        })
    }

    pub fn from_target(triple: &str) -> Result<Self> {
        Ok(match triple {
            "aarch64-apple-ios" => Self::App,
            "aarch64-linux-android" => Self::Apk,
            "x86_64-apple-darwin" => Self::App,
            "x86_64-pc-windows-msvc" => Self::Msix,
            "x86_64-unknown-linux-gnu" => Self::Appimage,
            target => anyhow::bail!("unsupported target {}", target),
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Mode {
    Cargo,
    Flutter,
}
