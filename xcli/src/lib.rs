use crate::devices::Device;
use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use xcommon::Signer;

pub mod android;
pub mod cargo;
pub mod config;
pub mod devices;
pub mod flutter;
pub mod maven;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Platform {
    Android,
    Ios,
    Linux,
    Macos,
    Windows,
}

impl Platform {
    pub fn host() -> Result<Self> {
        Ok(if cfg!(target_os = "linux") {
            Platform::Linux
        } else if cfg!(target_os = "macos") {
            Platform::Macos
        } else if cfg!(target_os = "windows") {
            Platform::Windows
        } else {
            anyhow::bail!("unsupported host");
        })
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Android => write!(f, "android"),
            Self::Ios => write!(f, "ios"),
            Self::Linux => write!(f, "linux"),
            Self::Macos => write!(f, "macos"),
            Self::Windows => write!(f, "windows"),
        }
    }
}

impl std::str::FromStr for Platform {
    type Err = anyhow::Error;

    fn from_str(platform: &str) -> Result<Self> {
        Ok(match platform {
            "android" => Self::Android,
            "ios" => Self::Ios,
            "linux" => Self::Linux,
            "macos" => Self::Macos,
            "windows" => Self::Windows,
            _ => anyhow::bail!("unsupported platform {}", platform),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arch {
    //Arm,
    Arm64,
    X64,
    //X86,
}

impl Arch {
    pub fn host() -> Result<Self> {
        if cfg!(target_arch = "x86_64") {
            Ok(Arch::X64)
        } else if cfg!(target_arch = "aarch64") {
            Ok(Arch::Arm64)
        } else {
            anyhow::bail!("unsupported host");
        }
    }
}

impl std::fmt::Display for Arch {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            //Self::Arm => write!(f, "arm"),
            Self::Arm64 => write!(f, "arm64"),
            Self::X64 => write!(f, "x64"),
            //Self::X86 => write!(f, "x86"),
        }
    }
}

impl std::str::FromStr for Arch {
    type Err = anyhow::Error;

    fn from_str(arch: &str) -> Result<Self> {
        Ok(match arch {
            //"arm" => Self::Arm,
            "arm64" => Self::Arm64,
            "x64" => Self::X64,
            //"x86" => Self::X86,
            _ => anyhow::bail!("unsupported arch {}", arch),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    Aab,
    Apk,
    Appimage,
    Dmg,
    Ipa,
    Msix,
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Aab => write!(f, "aab"),
            Self::Apk => write!(f, "apk"),
            Self::Appimage => write!(f, "appimage"),
            Self::Dmg => write!(f, "dmg"),
            Self::Ipa => write!(f, "ipa"),
            Self::Msix => write!(f, "msix"),
        }
    }
}

impl std::str::FromStr for Format {
    type Err = anyhow::Error;

    fn from_str(arch: &str) -> Result<Self> {
        Ok(match arch {
            "aab" => Self::Aab,
            "apk" => Self::Apk,
            "appimage" => Self::Appimage,
            "dmg" => Self::Dmg,
            "ipa" => Self::Ipa,
            "msix" => Self::Msix,
            _ => anyhow::bail!("unsupported arch {}", arch),
        })
    }
}

impl Format {
    pub fn platform_default(platform: Platform) -> Self {
        match platform {
            Platform::Android => Self::Apk,
            Platform::Ios => Self::Ipa,
            Platform::Linux => Self::Appimage,
            Platform::Macos => Self::Dmg,
            Platform::Windows => Self::Msix,
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Self::Aab => "aab",
            Self::Apk => "apk",
            Self::Appimage => "AppImage",
            Self::Dmg => "dmg",
            Self::Ipa => "ipa",
            Self::Msix => "msix",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Store {
    Apple,
    Microsoft,
    Play,
    Sideload,
}

impl std::fmt::Display for Store {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Apple => write!(f, "apple"),
            Self::Microsoft => write!(f, "microsoft"),
            Self::Play => write!(f, "play"),
            Self::Sideload => write!(f, "sideload"),
        }
    }
}

impl std::str::FromStr for Store {
    type Err = anyhow::Error;

    fn from_str(store: &str) -> Result<Self> {
        Ok(match store {
            "apple" => Self::Apple,
            "microsoft" => Self::Microsoft,
            "play" => Self::Play,
            "sideload" => Self::Sideload,
            _ => anyhow::bail!("unsupported store {}", store),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompileTarget {
    platform: Platform,
    arch: Arch,
    opt: Opt,
}

impl CompileTarget {
    pub fn new(platform: Platform, arch: Arch, opt: Opt) -> Self {
        Self {
            platform,
            arch,
            opt,
        }
    }

    pub fn platform(&self) -> Platform {
        self.platform
    }

    pub fn arch(&self) -> Arch {
        self.arch
    }

    pub fn opt(&self) -> Opt {
        self.opt
    }

    pub fn rust_triple(&self) -> Result<&'static str> {
        Ok(match (self.arch, self.platform) {
            (Arch::Arm64, Platform::Android) => "aarch64-linux-android",
            (Arch::Arm64, Platform::Ios) => "aarch64-apple-ios",
            (Arch::Arm64, Platform::Linux) => "aarch64-unknown-linux-gnu",
            (Arch::Arm64, Platform::Macos) => "aarch64-apple-darwin",
            (Arch::X64, Platform::Linux) => "x86_64-unknown-linux-gnu",
            (Arch::X64, Platform::Macos) => "x86_64-apple-darwin",
            (Arch::X64, Platform::Windows) => "x86_64-pc-windows-msvc",
            (arch, platform) => anyhow::bail!(
                "unsupported arch/platform combination {} {}",
                arch,
                platform
            ),
        })
    }
}

#[derive(Parser)]
pub struct BuildArgs {
    #[clap(long, conflicts_with = "release")]
    debug: bool,
    #[clap(long, conflicts_with = "debug")]
    release: bool,
    #[clap(long, conflicts_with = "device")]
    platform: Option<Platform>,
    #[clap(long, requires = "platform")]
    arch: Option<Arch>,
    #[clap(long, conflicts_with = "store")]
    device: Option<Device>,
    #[clap(long, conflicts_with = "device")]
    store: Option<Store>,
    #[clap(long, requires = "cert")]
    key: Option<PathBuf>,
    #[clap(long, requires = "key")]
    cert: Option<PathBuf>,
}

impl BuildArgs {
    pub fn build_target(self) -> Result<BuildTarget> {
        let signer = if let (Some(key), Some(cert)) = (self.key.as_ref(), self.cert.as_ref()) {
            let key = std::fs::read_to_string(key)?;
            let cert = std::fs::read_to_string(cert)?;
            Some(Signer::new(&key, &cert)?)
        } else {
            None
        };
        let store = self.store;
        let device = if self.platform.is_none() && store.is_none() && self.device.is_none() {
            Some(Device::host())
        } else {
            self.device
        };
        let platform = if let Some(platform) = self.platform {
            platform
        } else if let Some(store) = store {
            match store {
                Store::Apple => anyhow::bail!("apple store requires platform arg"),
                Store::Microsoft => Platform::Windows,
                Store::Play => Platform::Android,
                Store::Sideload => anyhow::bail!("sideload store requires platform arg"),
            }
        } else if let Some(device) = device.as_ref() {
            device.platform()?
        } else {
            unreachable!();
        };
        let archs = if let Some(arch) = self.arch {
            vec![arch]
        } else if let Some(store) = store {
            match store {
                Store::Apple => vec![Arch::X64, Arch::Arm64],
                Store::Microsoft => vec![Arch::X64],
                Store::Play => vec![Arch::Arm64],
                Store::Sideload => anyhow::bail!("sideload store requires arch arg"),
            }
        } else if let Some(device) = device.as_ref() {
            vec![device.arch()?]
        } else {
            unreachable!();
        };
        let format = if store == Some(Store::Play) {
            Format::Aab
        } else {
            Format::platform_default(platform)
        };
        let opt = if self.release || (!self.debug && self.store.is_some()) {
            Opt::Release
        } else {
            Opt::Debug
        };
        Ok(BuildTarget {
            opt,
            platform,
            archs,
            format,
            device,
            store,
            signer,
        })
    }
}

#[derive(Clone, Debug)]
pub struct BuildTarget {
    opt: Opt,
    platform: Platform,
    archs: Vec<Arch>,
    format: Format,
    device: Option<Device>,
    store: Option<Store>,
    signer: Option<Signer>,
}

impl BuildTarget {
    pub fn opt(&self) -> Opt {
        self.opt
    }

    pub fn platform(&self) -> Platform {
        self.platform
    }

    pub fn archs(&self) -> &[Arch] {
        &self.archs
    }

    pub fn format(&self) -> Format {
        self.format
    }

    pub fn device(&self) -> Option<&Device> {
        self.device.as_ref()
    }

    pub fn store(&self) -> Option<Store> {
        self.store
    }

    pub fn signer(&self) -> Option<&Signer> {
        self.signer.as_ref()
    }

    pub fn compile_targets(&self) -> impl Iterator<Item = CompileTarget> + '_ {
        self.archs
            .iter()
            .map(|arch| CompileTarget::new(self.platform, *arch, self.opt))
    }

    pub fn is_host(&self) -> bool {
        self.device
            .as_ref()
            .map(|device| device.is_host())
            .unwrap_or_default()
    }
}
