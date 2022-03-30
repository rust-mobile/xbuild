use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Default, Deserialize, Serialize)]
struct PubspecYaml {
    flutter: Option<FlutterSection>,
}

#[derive(Default, Deserialize, Serialize)]
struct FlutterSection {
    #[serde(rename = "uses-material-design")]
    #[serde(default)]
    uses_material_design: bool,
    #[serde(default)]
    assets: Vec<PathBuf>,
    #[serde(default)]
    fonts: Vec<FontFamily>,
}

impl FlutterSection {
    fn assets(&self) -> impl Iterator<Item = &Path> + '_ {
        self.assets
            .iter()
            .chain(
                self.fonts
                    .iter()
                    .map(|family| &family.fonts)
                    .flatten()
                    .map(|font| &font.asset),
            )
            .map(|path| path.as_path())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct FontFamily {
    family: String,
    fonts: Vec<Font>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct Font {
    asset: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    weight: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    style: Option<String>,
}

#[derive(Clone, Debug)]
struct Asset {
    root_dir: PathBuf,
    package: Option<String>,
    asset: PathBuf,
}

impl Asset {
    fn src(&self) -> PathBuf {
        self.root_dir.join(&self.asset)
    }

    fn dst(&self) -> PathBuf {
        if let Some(package) = self.package.as_ref() {
            Path::new("packages").join(package).join(&self.asset)
        } else {
            self.asset.clone()
        }
    }
}

#[derive(Deserialize, Serialize)]
struct PackageConfig {
    packages: Vec<Package>,
}

#[derive(Deserialize, Serialize)]
struct Package {
    name: String,
    #[serde(rename = "rootUri")]
    root_uri: String,
}

type AssetManifest = BTreeMap<PathBuf, Vec<PathBuf>>;
type FontManifest = Vec<FontFamily>;

#[derive(Clone, Debug)]
pub struct AssetBundle {
    assets: Vec<Vec<Asset>>,
    fonts: Vec<FontFamily>,
}

impl AssetBundle {
    pub fn new(root_dir: &Path, material_icons: &Path) -> Result<Self> {
        let mut bundle = Self {
            assets: vec![],
            fonts: FontManifest::default(),
        };
        bundle.add_pubspec_assets(root_dir, None, material_icons)?;
        let packages = root_dir.join(".dart_tool").join("package_config.json");
        let packages = std::fs::read_to_string(packages)?;
        let pconf: PackageConfig = serde_json::from_str(&packages)?;
        for package in pconf.packages {
            if let Some(path) = package.root_uri.strip_prefix("file://") {
                let name = package.name;
                bundle.add_pubspec_assets(Path::new(path), Some(name), material_icons)?;
            }
        }
        Ok(bundle)
    }

    fn add_pubspec_assets(
        &mut self,
        root_dir: &Path,
        package: Option<String>,
        material_icons: &Path,
    ) -> Result<()> {
        log::info!("processing {}", root_dir.display());
        let yaml = std::fs::read_to_string(root_dir.join("pubspec.yaml"))?;
        let yaml: PubspecYaml = serde_yaml::from_str(&yaml)?;
        let mut yaml = yaml.flutter.unwrap_or_default();

        for path in yaml.assets() {
            let asset = Asset {
                root_dir: root_dir.to_path_buf(),
                package: package.clone(),
                asset: path.to_path_buf(),
            };
            let asset_name = path.file_name().unwrap().to_str().unwrap();
            let asset_path = asset.src();
            let mut variants = vec![asset];
            for entry in std::fs::read_dir(asset_path.parent().unwrap())? {
                let entry = entry?;
                let variant = entry.path().join(asset_name);
                if !variant.exists() {
                    continue;
                }
                if !entry.file_type()?.is_file() {
                    continue;
                }
                variants.push(Asset {
                    root_dir: root_dir.to_path_buf(),
                    package: package.clone(),
                    asset: variant.strip_prefix(&root_dir)?.to_path_buf(),
                });
            }
            self.assets.push(variants);
        }

        if package.is_none() && yaml.uses_material_design {
            log::info!("adding material icons");
            let asset = Path::new("MaterialIcons-Regular.otf");
            self.assets.push(vec![Asset {
                root_dir: material_icons.to_path_buf(),
                package: None,
                asset: asset.to_path_buf(),
            }]);
            self.fonts.push(FontFamily {
                family: "MaterialIcons".into(),
                fonts: vec![Font {
                    asset: asset.to_path_buf(),
                    weight: None,
                    style: None,
                }],
            });
        }

        if let Some(package) = package {
            let package = Path::new("packages").join(package);
            for family in &mut yaml.fonts {
                for font in &mut family.fonts {
                    font.asset = package.join(&font.asset);
                }
            }
        }
        self.fonts.extend(yaml.fonts);

        Ok(())
    }

    fn asset_manifest(&self) -> AssetManifest {
        let mut manifest = AssetManifest::default();
        for variants in &self.assets {
            let asset = variants[0].dst();
            let variants = variants.iter().map(|variant| variant.dst()).collect();
            manifest.insert(asset, variants);
        }
        manifest
    }

    pub fn assemble(&self, target_dir: &Path) -> Result<()> {
        log::info!("assembling bundle");
        std::fs::create_dir_all(target_dir)?;
        for variants in &self.assets {
            for asset in variants {
                let src = asset.src();
                let dst = target_dir.join(asset.dst());
                std::fs::create_dir_all(dst.parent().unwrap())?;
                std::fs::copy(src, dst)?;
            }
        }
        let asset_manifest = serde_json::to_string(&self.asset_manifest())?;
        std::fs::write(target_dir.join("AssetManifest.json"), asset_manifest)?;
        let font_manifest = serde_json::to_string(&self.fonts)?;
        std::fs::write(target_dir.join("FontManifest.json"), font_manifest)?;
        Ok(())
    }
}
