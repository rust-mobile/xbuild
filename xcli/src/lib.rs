use anyhow::Result;
use std::path::Path;
use std::process::Command;

pub mod cargo;
pub mod config;
pub mod devices;
pub mod sdk;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Opt {
    Debug,
    Release,
}

impl std::fmt::Display for Opt {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Debug => write!(f, "debug"),
            Self::Release => write!(f, "release"),
        }
    }
}

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

pub fn flutter_build(target: &str, debug: bool) -> Result<()> {
    let mut cmd = Command::new("flutter");
    cmd.arg("build").arg(target);
    if debug {
        cmd.arg("--debug");
    }
    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("failed to run flutter");
    }
    Ok(())
}

pub fn display_cert_name(name: &rasn_pkix::Name) -> Result<String> {
    use rasn::prelude::Oid;
    let rasn_pkix::Name::RdnSequence(seq) = name;
    let mut attrs = vec![];
    for set in seq {
        for attr in set {
            let name = match &attr.r#type {
                ty if Oid::JOINT_ISO_ITU_T_DS_ATTRIBUTE_TYPE_COMMON_NAME == *ty => "CN",
                ty if Oid::JOINT_ISO_ITU_T_DS_ATTRIBUTE_TYPE_COUNTRY_NAME == *ty => "C",
                ty if Oid::JOINT_ISO_ITU_T_DS_ATTRIBUTE_TYPE_LOCALITY_NAME == *ty => "L",
                ty if Oid::JOINT_ISO_ITU_T_DS_ATTRIBUTE_TYPE_STATE_OR_PROVINCE_NAME == *ty => "ST",
                ty if Oid::JOINT_ISO_ITU_T_DS_ATTRIBUTE_TYPE_ORGANISATION_NAME == *ty => "O",
                ty if Oid::JOINT_ISO_ITU_T_DS_ATTRIBUTE_TYPE_ORGANISATIONAL_UNIT_NAME == *ty => {
                    "OU"
                }
                oid => unimplemented!("{:?}", oid),
            };
            attrs.push(format!(
                "{}={}",
                name,
                std::str::from_utf8(&attr.value.as_bytes()[2..])?
            ));
        }
    }
    Ok(attrs.join(" "))
}
