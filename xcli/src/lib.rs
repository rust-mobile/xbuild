use crate::android::{AndroidNdk, AndroidSdk};
use crate::cargo::{Cargo, CargoBuild, CrateType};
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

#[macro_export]
macro_rules! exe {
    ($name:expr) => {
        if cfg!(target_os = "windows") {
            concat!($name, ".exe")
        } else {
            $name
        }
    };
}

pub mod android;
pub mod cargo;
pub mod config;
pub mod devices;
pub mod download;
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
    #[clap(flatten)]
    build_target: BuildTargetArgs,
    #[clap(flatten)]
    cargo: CargoArgs,
}

#[derive(Parser)]
pub struct CargoArgs {
    #[clap(long, short)]
    package: Option<String>,
    #[clap(long)]
    manifest_path: Option<PathBuf>,
    #[clap(long)]
    target_dir: Option<PathBuf>,
}

impl CargoArgs {
    pub fn cargo(self) -> Result<Cargo> {
        Cargo::new(self.package.as_deref(), self.manifest_path, self.target_dir)
    }
}

#[derive(Parser)]
pub struct BuildTargetArgs {
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
    #[clap(long)]
    pem: Option<PathBuf>,
    #[clap(long)]
    provisioning_profile: Option<PathBuf>,
}

impl BuildTargetArgs {
    pub fn build_target(self) -> Result<BuildTarget> {
        let signer = if let Some(pem) = self.pem.as_ref() {
            if !pem.exists() {
                anyhow::bail!("pem file doesn't exist {}", pem.display());
            }
            Some(Signer::from_path(&pem)?)
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
        let provisioning_profile =
            if self.provisioning_profile.is_some() || platform == Platform::Ios {
                if self.provisioning_profile.is_some() && platform == Platform::Ios {
                    if let Some(provisioning_profile) = self.provisioning_profile.as_ref() {
                        if !provisioning_profile.exists() {
                            anyhow::bail!(
                                "provisioning profile doesn't exist {}",
                                provisioning_profile.display()
                            );
                        }
                    }
                    self.provisioning_profile
                } else {
                    anyhow::bail!("--provisioning-profile is only valid for ios");
                }
            } else {
                None
            };
        Ok(BuildTarget {
            opt,
            platform,
            archs,
            format,
            device,
            store,
            signer,
            provisioning_profile,
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
    provisioning_profile: Option<PathBuf>,
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

    pub fn signer(&self) -> Option<&Signer> {
        self.signer.as_ref()
    }

    pub fn provisioning_profile(&self) -> Option<&Path> {
        self.provisioning_profile.as_deref()
    }
}

pub struct BuildEnv {
    name: String,
    build_target: BuildTarget,
    build_dir: PathBuf,
    icon: Option<PathBuf>,
    target_file: PathBuf,
    cargo: Cargo,
    pubspec: PathBuf,
    android_manifest: Option<AndroidManifest>,
    appx_manifest: Option<AppxManifest>,
    info_plist: Option<InfoPlist>,
    flutter: Option<Flutter>,
    android_sdk: Option<AndroidSdk>,
    android_ndk: Option<AndroidNdk>,
}

impl BuildEnv {
    pub fn new(args: BuildArgs) -> Result<Self> {
        let cargo = args.cargo.cargo()?;
        let build_target = args.build_target.build_target()?;
        let build_dir = cargo.target_dir().join("x");
        let pubspec = cargo.root_dir().join("pubspec.yaml");
        let flutter = if pubspec.exists() {
            Some(Flutter::new(build_dir.join("Flutter.sdk"))?)
        } else {
            None
        };
        let config = if flutter.is_some() {
            Config::parse(&pubspec)?
        } else {
            Config::parse(cargo.manifest())?
        };
        let android_sdk = if build_target.platform() == Platform::Android {
            Some(AndroidSdk::from_env()?)
        } else {
            None
        };
        let android_ndk = android_sdk.as_ref().map(AndroidNdk::from_env).transpose()?;
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
        let target_file = config.target_file(cargo.root_dir(), build_target.platform());
        let icon = config
            .icon(build_target.format())
            .map(|icon| cargo.root_dir().join(icon));
        let name = config.name;
        Ok(Self {
            name,
            build_target,
            pubspec,
            target_file,
            icon,
            cargo,
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

    pub fn has_dart_code(&self) -> bool {
        self.flutter.is_some()
    }

    pub fn pubspec(&self) -> &Path {
        &self.pubspec
    }

    pub fn root_dir(&self) -> &Path {
        self.cargo.root_dir()
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

    pub fn cargo(&self) -> &Cargo {
        &self.cargo
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

    pub fn cargo_build(&self, target: CompileTarget, target_dir: &Path) -> Result<CargoBuild> {
        let mut cargo = self.cargo.build(target, target_dir)?;
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
        if target.platform() == Platform::Ios {
            let sdk = self.build_dir().join("iPhoneOS.sdk");
            if sdk.exists() {
                cargo.use_ios_sdk(&sdk)?;
            }
        }
        if let Some(flutter) = self.flutter() {
            match self.target().platform() {
                Platform::Linux => {
                    cargo.add_lib_dir(&flutter.engine_dir(target)?);
                }
                Platform::Macos => {
                    cargo.add_framework_dir(&flutter.engine_dir(target)?);
                }
                Platform::Windows => {
                    cargo.add_lib_dir(&flutter.engine_dir(target)?);
                }
                _ => {}
            }
        }
        Ok(cargo)
    }

    pub fn cargo_artefact(&self, target_dir: &Path, target: CompileTarget) -> Result<PathBuf> {
        self.cargo
            .artifact(target_dir, target, None, CrateType::Bin)
    }

    pub fn maven(&self) -> Result<Maven> {
        let mut maven = Maven::new(self.build_dir.join("maven"))?;
        maven.add_repository(crate::maven::GOOGLE);
        maven.add_repository(crate::maven::FLUTTER);
        maven.add_repository(crate::maven::CENTRAL);
        Ok(maven)
    }
}
