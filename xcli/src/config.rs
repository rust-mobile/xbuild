use crate::{Opt, Platform};
use anyhow::Result;
use appbundle::InfoPlist;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use xapk::manifest::{Activity, AndroidManifest, IntentFilter, MetaData, Permission};
use xapk::VersionCode;
use xmsix::AppxManifest;

#[derive(Clone, Debug)]
pub struct Config {
    pub name: String,
    pub version: String,
    pub description: String,
}

impl Config {
    pub fn cargo_toml(path: &Path) -> Result<Self> {
        CargoToml::parse(path)
    }

    pub fn pubspec_yaml(path: &Path) -> Result<Self> {
        PubspecYaml::parse(path)
    }
}

#[derive(Deserialize)]
struct CargoToml {
    package: Package,
}

#[derive(Deserialize)]
struct Package {
    name: String,
    version: String,
    description: Option<String>,
}

impl CargoToml {
    pub fn parse(path: &Path) -> Result<Config> {
        let contents = std::fs::read_to_string(path)?;
        let toml: CargoToml = toml::from_str(&contents)?;
        Ok(Config {
            name: toml.package.name,
            version: toml.package.version,
            description: toml.package.description.unwrap_or_default(),
        })
    }
}

#[derive(Deserialize)]
struct PubspecYaml {
    name: String,
    version: String,
    description: Option<String>,
}

impl PubspecYaml {
    pub fn parse(path: &Path) -> Result<Config> {
        let contents = std::fs::read_to_string(path)?;
        let yaml: PubspecYaml = serde_yaml::from_str(&contents)?;
        Ok(Config {
            name: yaml.name,
            version: yaml.version,
            description: yaml.description.unwrap_or_default(),
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct Manifest {
    generic: GenericConfig,
    android: ApkConfig,
    ios: AppbundleConfig,
    linux: AppimageConfig,
    macos: AppbundleConfig,
    windows: MsixConfig,
}

impl Manifest {
    pub fn parse<P: AsRef<Path>>(path: P) -> Result<Self> {
        if !path.as_ref().exists() {
            return Ok(Default::default());
        }
        let contents = std::fs::read_to_string(path.as_ref())?;
        let config: RawConfig = serde_yaml::from_str(&contents)?;
        Ok(Manifest {
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

    pub fn target_file(&self, path: &Path, platform: Platform) -> PathBuf {
        let file = path.join("lib").join(format!("{}.dart", platform));
        if file.exists() {
            file
        } else {
            path.join("lib").join("main.dart")
        }
    }

    pub fn apply_config(&mut self, config: &Config, opt: Opt, flutter: bool) {
        let manifest = &mut self.android.manifest;
        manifest
            .package
            .get_or_insert_with(|| format!("com.example.{}", config.name.replace('-', "_")));
        manifest
            .version_name
            .get_or_insert_with(|| config.version.clone());
        if let Ok(code) = VersionCode::from_semver(&config.version) {
            manifest.version_code.get_or_insert_with(|| code.to_code(1));
        }
        let target_sdk_version = 31;
        let target_sdk_codename = 11;
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
        if flutter && opt == Opt::Debug {
            manifest.uses_permission.push(Permission {
                name: "android.permission.INTERNET".into(),
                max_sdk_version: None,
            });
        }

        let application = &mut manifest.application;
        application.label.get_or_insert_with(|| config.name.clone());
        application
            .debuggable
            .get_or_insert_with(|| opt == Opt::Debug);
        if flutter {
            application
                .theme
                .get_or_insert_with(|| "@android:style/Theme.Light.NoTitleBar".into());
            application
                .app_component_factory
                .get_or_insert_with(|| "androidx.core.app.CoreComponentFactory".into());
            application.meta_data.push(MetaData {
                name: "flutterEmbedding".into(),
                value: "2".into(),
            });
        } else {
            application.has_code.get_or_insert(false);
        }
        if application.activities.is_empty() {
            let activity = Activity {
                config_changes: Some(
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
                    .join("|"),
                ),
                label: None,
                launch_mode: Some("singleTop".into()),
                name: Some(if flutter {
                    "io.flutter.embedding.android.FlutterActivity".into()
                } else {
                    "android.app.NativeActivity".into()
                }),
                orientation: None,
                window_soft_input_mode: Some("adjustResize".into()),
                hardware_accelerated: Some(true),
                exported: Some(true),
                meta_data: if flutter {
                    vec![]
                } else {
                    vec![MetaData {
                        name: "android.app.lib_name".into(),
                        value: config.name.replace('-', "_"),
                    }]
                },
                intent_filters: vec![IntentFilter {
                    actions: vec!["android.intent.action.MAIN".into()],
                    categories: vec!["android.intent.category.LAUNCHER".into()],
                    data: vec![],
                }],
            };
            application.activities.push(activity);
        }

        self.ios
            .info
            .name
            .get_or_insert_with(|| config.name.clone());
        self.ios
            .info
            .short_version
            .get_or_insert_with(|| config.version.clone());
        self.ios.info.requires_ios.get_or_insert(true);
        self.ios
            .info
            .storyboard_name
            .get_or_insert_with(|| "".into());

        self.macos
            .info
            .name
            .get_or_insert_with(|| config.name.clone());
        self.macos
            .info
            .short_version
            .get_or_insert_with(|| config.version.clone());
        self.macos
            .info
            .minimum_system_version
            .get_or_insert_with(|| "10.11".to_string());

        self.windows
            .manifest
            .properties
            .display_name
            .get_or_insert_with(|| config.name.clone());
        self.windows
            .manifest
            .identity
            .version
            .get_or_insert_with(|| config.version.clone());
        self.windows
            .manifest
            .properties
            .description
            .get_or_insert_with(|| config.description.clone());
    }

    pub fn android(&self) -> &AndroidManifest {
        &self.android.manifest
    }

    pub fn ios(&self) -> &InfoPlist {
        &self.ios.info
    }

    pub fn macos(&self) -> &InfoPlist {
        &self.macos.info
    }

    pub fn windows(&self) -> &AppxManifest {
        &self.windows.manifest
    }
}

#[derive(Deserialize)]
struct RawConfig {
    #[serde(flatten)]
    generic: Option<GenericConfig>,
    android: Option<ApkConfig>,
    linux: Option<AppimageConfig>,
    ios: Option<AppbundleConfig>,
    macos: Option<AppbundleConfig>,
    windows: Option<MsixConfig>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct GenericConfig {
    icon: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct AppbundleConfig {
    #[serde(flatten)]
    generic: GenericConfig,
    info: InfoPlist,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ApkConfig {
    #[serde(flatten)]
    generic: GenericConfig,
    manifest: AndroidManifest,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct AppimageConfig {
    #[serde(flatten)]
    generic: GenericConfig,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MsixConfig {
    #[serde(flatten)]
    generic: GenericConfig,
    manifest: AppxManifest,
}
