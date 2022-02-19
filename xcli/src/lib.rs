use crate::android::{AndroidNdk, AndroidSdk};
use crate::cargo::Cargo;
use crate::config::Config;
use crate::devices::Device;
use crate::flutter::Flutter;
use crate::maven::Maven;
use anyhow::Result;
use appbundle::InfoPlist;
use clap::Parser;
use std::path::{Path, PathBuf};
use xapk::AndroidManifest;
use xcommon::Signer;
use xmsix::AppxManifest;

pub mod android;
pub mod cargo;
pub mod config;
pub mod devices;
pub mod flutter;
pub mod github;
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

    pub fn platform(self) -> Platform {
        self.platform
    }

    pub fn arch(self) -> Arch {
        self.arch
    }

    pub fn opt(self) -> Opt {
        self.opt
    }

    pub fn android_abi(self) -> Result<xapk::Target> {
        match (self.platform, self.arch) {
            (Platform::Android, Arch::Arm64) => Ok(xapk::Target::Arm64V8a),
            (Platform::Android, Arch::X64) => Ok(xapk::Target::X86_64),
            _ => anyhow::bail!("unsupported android abi"),
        }
    }

    pub fn rust_triple(self) -> Result<&'static str> {
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

impl std::fmt::Display for CompileTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}-{}-{}", self.platform, self.arch, self.opt)
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

pub struct BuildEnv {
    name: String,
    build_target: BuildTarget,
    build_dir: PathBuf,
    has_rust_code: bool,
    icon: Option<PathBuf>,
    target_file: PathBuf,
    android_manifest: Option<AndroidManifest>,
    appx_manifest: Option<AppxManifest>,
    info_plist: Option<InfoPlist>,
    flutter: Option<Flutter>,
    android_sdk: Option<AndroidSdk>,
    android_ndk: Option<AndroidNdk>,
}

impl BuildEnv {
    pub fn new(args: BuildArgs) -> Result<Self> {
        let build_target = args.build_target()?;
        let has_rust_code = Path::new("Cargo.toml").exists();
        let build_dir = Path::new("target").join("x");
        let flutter = if Path::new("pubspec.yaml").exists() {
            Some(Flutter::from_env()?)
        } else {
            None
        };
        let config = if flutter.is_some() {
            Config::parse("pubspec.yaml")?
        } else {
            Config::parse("Cargo.toml")?
        };
        let android_sdk = if build_target.platform() == Platform::Android {
            Some(AndroidSdk::from_env()?)
        } else {
            None
        };
        let android_ndk = if let Some(sdk) = android_sdk.as_ref() {
            if has_rust_code {
                Some(AndroidNdk::from_env(sdk)?)
            } else {
                None
            }
        } else {
            None
        };
        let android_manifest = if let Some(sdk) = android_sdk.as_ref() {
            Some(config.android_manifest(&sdk)?)
        } else {
            None
        };
        let appx_manifest = if build_target.platform() == Platform::Windows {
            Some(config.appx_manifest()?)
        } else {
            None
        };
        let info_plist = if build_target.platform() == Platform::Macos
            || build_target.platform() == Platform::Ios
        {
            Some(config.info_plist()?)
        } else {
            None
        };
        let target_file = config.target_file(build_target.platform());
        let icon = config
            .icon(build_target.format())
            .map(|icon| icon.to_path_buf());
        let name = config.name;
        Ok(Self {
            name,
            build_target,
            has_rust_code,
            target_file,
            icon,
            flutter,
            android_sdk,
            android_ndk,
            android_manifest,
            appx_manifest,
            info_plist,
            build_dir,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn target(&self) -> &BuildTarget {
        &self.build_target
    }

    pub fn has_rust_code(&self) -> bool {
        self.has_rust_code
    }

    pub fn has_dart_code(&self) -> bool {
        self.flutter.is_some()
    }

    pub fn build_dir(&self) -> &Path {
        &self.build_dir
    }

    pub fn target_file(&self) -> &Path {
        &self.target_file
    }

    pub fn icon(&self) -> Option<&Path> {
        self.icon.as_deref()
    }

    pub fn flutter(&self) -> Option<&Flutter> {
        self.flutter.as_ref()
    }

    pub fn android_sdk(&self) -> Option<&AndroidSdk> {
        self.android_sdk.as_ref()
    }

    pub fn android_ndk(&self) -> Option<&AndroidNdk> {
        self.android_ndk.as_ref()
    }

    pub fn android_manifest(&self) -> Option<&AndroidManifest> {
        self.android_manifest.as_ref()
    }

    pub fn appx_manifest(&self) -> Option<&AppxManifest> {
        self.appx_manifest.as_ref()
    }

    pub fn info_plist(&self) -> Option<&InfoPlist> {
        self.info_plist.as_ref()
    }

    fn target_sdk_version(&self) -> u32 {
        self.android_manifest()
            .unwrap()
            .sdk
            .target_sdk_version
            .unwrap()
    }

    pub fn android_jar(&self) -> Result<PathBuf> {
        self.android_sdk()
            .unwrap()
            .android_jar(self.target_sdk_version())
    }

    pub fn cargo(&self, target: CompileTarget) -> Result<Cargo> {
        let mut cargo = Cargo::new(target)?;
        if let Some(ndk) = self.android_ndk() {
            cargo.use_ndk_tools(ndk, self.target_sdk_version())?;
        }
        if target.platform() == Platform::Windows {
            let sdk = self.build_dir().join("Windows.sdk");
            if sdk.exists() {
                cargo.use_xwin(&sdk)?;
            }
        }
        if target.platform() == Platform::Macos {
            let sdk = self.build_dir().join("MacOSX.sdk");
            if sdk.exists() {
                let minimum_version = self
                    .info_plist()
                    .unwrap()
                    .minimum_system_version
                    .as_ref()
                    .unwrap();
                cargo.use_macos_sdk(&sdk, minimum_version)?;
            }
        }
        if let Some(flutter) = self.flutter() {
            match self.target().platform() {
                Platform::Linux => {
                    cargo.add_lib_dir(&flutter.engine_dir(target)?);
                }
                Platform::Macos => {
                    cargo.add_framework_dir(&flutter.engine_dir(target)?);
                    cargo.link_framework("FlutterMacOS");
                }
                Platform::Windows => {
                    cargo.add_lib_dir(&flutter.engine_dir(target)?);
                    cargo.link_lib("flutter_windows.dll");
                }
                _ => {}
            }
        }
        Ok(cargo)
    }

    pub fn maven(&self) -> Result<Maven> {
        Maven::new(self.build_dir.join("maven"))
    }

    pub fn cargo_artefact(&self, target: CompileTarget) -> Result<PathBuf> {
        let target_dir = Path::new("target");
        let arch_dir = if target.platform() == Platform::host()? && target.arch() == Arch::host()? {
            target_dir.to_path_buf()
        } else {
            target_dir.join(target.rust_triple()?)
        };
        let opt_dir = arch_dir.join(target.opt().to_string());
        let bin_name = if target.platform() == Platform::Windows {
            format!("{}.exe", self.name())
        } else {
            self.name.clone()
        };
        let bin_path = opt_dir.join(bin_name);
        if !bin_path.exists() {
            anyhow::bail!("failed to locate bin {}", bin_path.display());
        }
        Ok(bin_path)
    }
}
