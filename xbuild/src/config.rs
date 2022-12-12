use crate::cargo::manifest::{Inheritable, Manifest, Package};
use crate::{Opt, Platform};
use anyhow::{Context, Result};
use apk::manifest::{Activity, AndroidManifest, IntentFilter, MetaData};
use apk::VersionCode;
use appbundle::InfoPlist;
use msix::AppxManifest;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default)]
pub struct Config {
    generic: GenericConfig,
    android: AndroidConfig,
    ios: IosConfig,
    linux: LinuxConfig,
    macos: MacosConfig,
    windows: WindowsConfig,
}

impl Config {
    pub fn parse<P: AsRef<Path>>(path: P) -> Result<Self> {
        if !path.as_ref().exists() {
            return Ok(Default::default());
        }
        let contents = std::fs::read_to_string(path.as_ref())?;
        let config: RawConfig = serde_yaml::from_str(&contents)?;
        Ok(Self {
            generic: config.generic.unwrap_or_default(),
            android: config.android.unwrap_or_default(),
            ios: config.ios.unwrap_or_default(),
            linux: config.linux.unwrap_or_default(),
            macos: config.macos.unwrap_or_default(),
            windows: config.windows.unwrap_or_default(),
        })
    }

    pub fn icon(&self, platform: Platform) -> Option<&Path> {
        let icon = match platform {
            Platform::Android => self.android.generic.icon.as_deref(),
            Platform::Ios => self.ios.generic.icon.as_deref(),
            Platform::Macos => self.macos.generic.icon.as_deref(),
            Platform::Linux => self.linux.generic.icon.as_deref(),
            Platform::Windows => self.windows.generic.icon.as_deref(),
        };
        if let Some(icon) = icon {
            return Some(icon);
        }
        self.generic.icon.as_deref()
    }

    pub fn apply_rust_package(
        &mut self,
        manifest_package: &Package,
        workspace_manifest: Option<&Manifest>,
        opt: Opt,
    ) -> Result<()> {
        let wry = self.android.wry;
        if wry {
            self.android
                .dependencies
                .push("androidx.appcompat:appcompat:1.4.1".into());
        }
        let manifest = &mut self.android.manifest;
        manifest.package.get_or_insert_with(|| {
            format!("com.example.{}", manifest_package.name.replace('-', "_"))
        });

        let inherit_package_field = || {
            let workspace = workspace_manifest
                .context("`workspace=true` requires a workspace")?
                .workspace
                .as_ref()
                // Unreachable:
                .expect("Caller-provided workspace lacks `[workspace]` table");

            workspace.package.as_ref().context("Failed to inherit field: `workspace.package` was not defined in workspace root manifest")
        };

        let package_version = match &manifest_package.version {
            Inheritable::Value(v) => v.clone(),
            Inheritable::Inherited { workspace: true } => inherit_package_field()?
                .version
                .clone()
                .context("Failed to inherit field: `workspace.package.version` was not defined in workspace root manifest")?,
            Inheritable::Inherited { workspace: false } => {
                anyhow::bail!("`workspace=false` is unsupported")
            }
        };

        let package_description = match &manifest_package.description {
            Some(Inheritable::Value(v)) => v.clone(),
            Some(Inheritable::Inherited { workspace: true }) => inherit_package_field()?
                .description
                .clone()
                .context("Failed to inherit field: `workspace.package.description` was not defined in workspace root manifest")?,
            Some(Inheritable::Inherited { workspace: false }) => {
                anyhow::bail!("`workspace=false` is unsupported")
            }
            None => "".into(),
        };

        manifest
            .version_name
            .get_or_insert_with(|| package_version.clone());
        if let Ok(code) = VersionCode::from_semver(&package_version) {
            manifest.version_code.get_or_insert_with(|| code.to_code(1));
        }
        let target_sdk_version = 33;
        let target_sdk_codename = 13;
        let min_sdk_version = 21;
        manifest
            .compile_sdk_version
            .get_or_insert(target_sdk_version);
        manifest
            .platform_build_version_code
            .get_or_insert(target_sdk_version);
        manifest
            .compile_sdk_version_codename
            .get_or_insert(target_sdk_codename);
        manifest
            .platform_build_version_name
            .get_or_insert(target_sdk_codename);
        manifest
            .sdk
            .target_sdk_version
            .get_or_insert(target_sdk_version);
        manifest.sdk.min_sdk_version.get_or_insert(min_sdk_version);

        let application = &mut manifest.application;
        application
            .label
            .get_or_insert_with(|| manifest_package.name.clone());
        if wry {
            application
                .theme
                .get_or_insert_with(|| "@style/Theme.AppCompat.Light.NoActionBar".into());
        }
        application
            .debuggable
            .get_or_insert_with(|| opt == Opt::Debug);
        application.has_code.get_or_insert(wry);

        if application.activities.is_empty() {
            application.activities.push(Activity::default());
        }

        let activity = application.activities.get_mut(0).unwrap();
        activity.config_changes.get_or_insert_with(|| {
            [
                "orientation",
                "keyboardHidden",
                "keyboard",
                "screenSize",
                "smallestScreenSize",
                "locale",
                "layoutDirection",
                "fontScale",
                "screenLayout",
                "density",
                "uiMode",
            ]
            .join("|")
        });
        activity
            .launch_mode
            .get_or_insert_with(|| "singleTop".into());
        activity.name.get_or_insert_with(|| {
            if wry {
                ".MainActivity".into()
            } else {
                "android.app.NativeActivity".into()
            }
        });
        activity
            .window_soft_input_mode
            .get_or_insert_with(|| "adjustResize".into());
        activity.hardware_accelerated.get_or_insert(true);
        activity.exported.get_or_insert(true);
        if !wry {
            activity.meta_data.push(MetaData {
                name: "android.app.lib_name".into(),
                value: manifest_package.name.replace('-', "_"),
            });
        }
        activity.intent_filters.push(IntentFilter {
            actions: vec!["android.intent.action.MAIN".into()],
            categories: vec!["android.intent.category.LAUNCHER".into()],
            data: vec![],
        });

        self.ios
            .info
            .name
            .get_or_insert_with(|| manifest_package.name.clone());
        self.ios
            .info
            .bundle_identifier
            .get_or_insert_with(|| manifest_package.name.clone());
        self.ios
            .info
            .version
            .get_or_insert_with(|| package_version.clone());
        self.ios
            .info
            .short_version
            .get_or_insert_with(|| package_version.clone());
        self.ios
            .info
            .minimum_os_version
            .get_or_insert_with(|| "10.0".to_string());
        self.ios.info.requires_ios.get_or_insert(true);
        self.ios
            .info
            .storyboard_name
            .get_or_insert_with(|| "".into());
        self.ios
            .info
            .platform_name
            .get_or_insert_with(|| "iphoneos".into());
        self.ios
            .info
            .required_device_capabilities
            .get_or_insert_with(|| vec!["arm64".into()]);
        self.ios
            .info
            .device_family
            .get_or_insert_with(|| vec![1, 2]);
        self.ios
            .info
            .package_type
            .get_or_insert_with(|| "APPL".into());

        self.macos
            .info
            .name
            .get_or_insert_with(|| manifest_package.name.clone());
        self.macos
            .info
            .short_version
            .get_or_insert_with(|| package_version.clone());
        self.macos
            .info
            .minimum_system_version
            .get_or_insert_with(|| "10.11".to_string());

        self.windows
            .manifest
            .properties
            .display_name
            .get_or_insert_with(|| manifest_package.name.clone());
        self.windows
            .manifest
            .identity
            .version
            .get_or_insert(package_version);
        self.windows
            .manifest
            .properties
            .description
            .get_or_insert(package_description);

        Ok(())
    }

    pub fn android(&self) -> &AndroidConfig {
        &self.android
    }

    pub fn linux(&self) -> &LinuxConfig {
        &self.linux
    }

    pub fn ios(&self) -> &IosConfig {
        &self.ios
    }

    pub fn macos(&self) -> &MacosConfig {
        &self.macos
    }

    pub fn windows(&self) -> &WindowsConfig {
        &self.windows
    }
}

#[derive(Deserialize)]
struct RawConfig {
    #[serde(flatten)]
    generic: Option<GenericConfig>,
    android: Option<AndroidConfig>,
    linux: Option<LinuxConfig>,
    ios: Option<IosConfig>,
    macos: Option<MacosConfig>,
    windows: Option<WindowsConfig>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct GenericConfig {
    icon: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct AndroidConfig {
    #[serde(flatten)]
    generic: GenericConfig,
    #[serde(default)]
    pub manifest: AndroidManifest,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default)]
    pub gradle: bool,
    #[serde(default)]
    pub wry: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct IosConfig {
    #[serde(flatten)]
    generic: GenericConfig,
    pub info: InfoPlist,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MacosConfig {
    #[serde(flatten)]
    generic: GenericConfig,
    pub info: InfoPlist,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct LinuxConfig {
    #[serde(flatten)]
    generic: GenericConfig,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct WindowsConfig {
    #[serde(flatten)]
    generic: GenericConfig,
    pub manifest: AppxManifest,
}
