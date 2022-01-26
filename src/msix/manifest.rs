use anyhow::Result;
use serde::ser::{SerializeTuple, Serializer};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename = "Package")]
pub struct AppxManifest {
    #[serde(rename(serialize = "xmlns"))]
    #[serde(default = "default_namespace")]
    ns: String,
    #[serde(rename(serialize = "xmlns:uap"))]
    #[serde(default = "default_uap_namespace")]
    ns_uap: String,
    #[serde(rename(serialize = "xmlns:rescap"))]
    #[serde(default = "default_rescap_namespace")]
    ns_rescap: String,
    #[serde(rename(serialize = "Identity"))]
    pub identity: Identity,
    #[serde(rename(serialize = "Properties"))]
    pub properties: Properties,
    #[serde(rename(serialize = "Resources"))]
    pub resources: Resources,
    #[serde(rename(serialize = "Dependencies"))]
    pub dependencies: Dependencies,
    #[serde(rename(serialize = "Capabilities"))]
    #[serde(serialize_with = "serialize_element")]
    pub capabilities: Vec<Capability>,
    #[serde(rename(serialize = "Applications"))]
    pub applications: Applications,
}

impl Default for AppxManifest {
    fn default() -> Self {
        Self {
            ns: default_namespace(),
            ns_uap: default_uap_namespace(),
            ns_rescap: default_rescap_namespace(),
            identity: Default::default(),
            properties: Default::default(),
            resources: Default::default(),
            dependencies: Default::default(),
            capabilities: Default::default(),
            applications: Default::default(),
        }
    }
}

macro_rules! elements {
    ($plural:ident, $singular:expr, $ty:ty) => {
        #[derive(Clone, Debug, Default, Deserialize, Serialize)]
        pub struct $plural {
            #[serde(rename(serialize = $singular))]
            inner: Vec<$ty>,
        }

        impl std::ops::Deref for $plural {
            type Target = Vec<$ty>;

            fn deref(&self) -> &Self::Target {
                &self.inner
            }
        }

        impl std::ops::DerefMut for $plural {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.inner
            }
        }

        impl From<Vec<$ty>> for $plural {
            fn from(inner: Vec<$ty>) -> Self {
                Self { inner }
            }
        }

        impl From<$plural> for Vec<$ty> {
            fn from(outer: $plural) -> Vec<$ty> {
                outer.inner
            }
        }
    };
}

elements!(Applications, "Application", Application);
elements!(Dependencies, "TargetDeviceFamily", TargetDeviceFamily);
elements!(Resources, "Resource", Resource);

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Identity {
    #[serde(rename(serialize = "Name"))]
    name: String,
    #[serde(rename(serialize = "Version"))]
    version: String,
    #[serde(rename(serialize = "Publisher"))]
    publisher: String,
    #[serde(rename(serialize = "ProcessorArchitecture"))]
    arch: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Properties {
    #[serde(rename(serialize = "DisplayName"))]
    #[serde(serialize_with = "serialize_element")]
    display_name: String,
    #[serde(rename(serialize = "PublisherDisplayName"))]
    #[serde(serialize_with = "serialize_element")]
    publisher_display_name: String,
    #[serde(rename(serialize = "Logo"))]
    #[serde(serialize_with = "serialize_element")]
    logo: String,
    #[serde(rename(serialize = "Description"))]
    #[serde(serialize_with = "serialize_element")]
    description: String,
}

fn serialize_element<S>(value: &impl Serialize, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut tuple = serializer.serialize_tuple(1)?;
    tuple.serialize_element(value)?;
    tuple.end()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Resource {
    #[serde(rename(serialize = "Language"))]
    #[serde(default = "default_language")]
    language: String,
}

impl Default for Resource {
    fn default() -> Self {
        Self {
            language: default_language(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TargetDeviceFamily {
    #[serde(rename(serialize = "Name"))]
    name: String,
    #[serde(rename(serialize = "MinVersion"))]
    min_version: String,
    #[serde(rename(serialize = "MaxVersionTested"))]
    max_version: String,
}

impl Default for TargetDeviceFamily {
    fn default() -> Self {
        Self {
            name: "Windows.Desktop".into(),
            min_version: "10.0.0.0".into(),
            max_version: "10.0.20348.0".into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Capability {
    Capability {
        #[serde(rename(serialize = "Name"))]
        name: String,
    },
    #[serde(rename(serialize = "rescap::Capability"))]
    Restricted {
        #[serde(rename(serialize = "Name"))]
        name: String,
    },
    #[serde(rename(serialize = "DeviceCapability"))]
    Device {
        #[serde(rename(serialize = "Name"))]
        name: String,
    },
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Application {
    #[serde(rename(serialize = "Id"))]
    pub id: String,
    #[serde(rename(serialize = "Executable"))]
    pub executable: String,
    #[serde(rename(serialize = "EntryPoint"))]
    pub entry_point: String,
    #[serde(rename(serialize = "uap:VisualElements"))]
    pub visual_elements: VisualElements,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct VisualElements {
    #[serde(rename(serialize = "BackgroundColor"))]
    pub background_color: String,
    #[serde(rename(serialize = "DisplayName"))]
    pub display_name: String,
    #[serde(rename(serialize = "Description"))]
    pub description: String,
    #[serde(rename(serialize = "Square150x150Logo"))]
    pub logo_150x150: String,
    #[serde(rename(serialize = "Square44x44Logo"))]
    pub logo_44x44: String,
    #[serde(rename(serialize = "uap:DefaultTile"))]
    pub default_tile: DefaultTile,
    #[serde(rename(serialize = "uap:SplashScreen"))]
    pub splash_screen: SplashScreen,
    #[serde(rename(serialize = "uap:LockScreen"))]
    pub lock_screen: LockScreen,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct DefaultTile {
    #[serde(rename(serialize = "ShortName"))]
    pub short_name: String,
    #[serde(rename(serialize = "Square71x71Logo"))]
    pub logo_71x71: String,
    #[serde(rename(serialize = "Square310x310Logo"))]
    pub logo_310x310: String,
    #[serde(rename(serialize = "Wide310x150Logo"))]
    pub logo_310x150: String,
    #[serde(rename(serialize = "uap:ShowNameOnTiles"))]
    pub show_names_on_tiles: ShowNameOnTiles,
}

elements!(ShowNameOnTiles, "ShowOn", ShowOn);

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ShowOn {
    #[serde(rename(serialize = "Tile"))]
    pub tile: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SplashScreen {
    #[serde(rename(serialize = "Image"))]
    pub image: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct LockScreen {
    #[serde(rename(serialize = "BadgeLogo"))]
    pub badge_logo: String,
    #[serde(rename(serialize = "Notification"))]
    #[serde(default = "lock_screen_notification")]
    pub notification: String,
}

fn default_namespace() -> String {
    "http://schemas.microsoft.com/appx/manifest/foundation/windows10".to_string()
}

fn default_uap_namespace() -> String {
    "http://schemas.microsoft.com/appx/manifest/uap/windows10".to_string()
}

fn default_rescap_namespace() -> String {
    "http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
        .to_string()
}

fn default_language() -> String {
    "en-us".to_string()
}

fn lock_screen_notification() -> String {
    "badge".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_properties() {
        let props = Properties {
            display_name: "".into(),
            publisher_display_name: "".into(),
            logo: "".into(),
            description: "".into(),
        };
        let xml = quick_xml::se::to_string(&props).unwrap();
        assert_eq!(xml, "<Properties><DisplayName></DisplayName><PublisherDisplayName></PublisherDisplayName><Logo></Logo><Description></Description></Properties>");
    }

    #[test]
    fn test_manifest() {
        let manifest = AppxManifest {
            identity: Identity {
                name: "com.flutter.fluttertodoapp".into(),
                version: "1.0.0.0".into(),
                publisher: "CN=Msix Testing, O=Msix Testing Corporation, S=Some-State, C=US".into(),
                arch: "x64".into(),
            },
            properties: Properties {
                display_name: "fluttertodoapp".into(),
                publisher_display_name: "com.flutter.fluttertodoapp".into(),
                logo: "Images\\StoreLogo.png".into(),
                description: "A new Flutter project.".into(),
            },
            resources: vec![Default::default()].into(),
            dependencies: vec![Default::default()].into(),
            capabilities: vec![
                Capability::Capability {
                    name: "internetClient".into(),
                },
                Capability::Restricted {
                    name: "runFullTrust".into(),
                },
                Capability::Device {
                    name: "location".into(),
                },
            ],
            applications: vec![Application {
                id: "fluttertodoapp".into(),
                executable: "todoapp.exe".into(),
                entry_point: "Windows.FullTrustApplication".into(),
                visual_elements: VisualElements {
                    background_color: "transparent".into(),
                    display_name: "fluttertodoapp".into(),
                    description: "A new flutter project.".into(),
                    logo_44x44: "Images\\Square44x44Logo.png".into(),
                    logo_150x150: "Images\\Square150x150Logo.png".into(),
                    default_tile: DefaultTile {
                        short_name: "fluttertodoapp".into(),
                        logo_71x71: "Images\\SmallTile.png".into(),
                        logo_310x310: "Images\\LargeTile.png".into(),
                        logo_310x150: "Images\\Wide310x150Logo.png".into(),
                        show_names_on_tiles: vec![
                            ShowOn {
                                tile: "square150x150Logo".into(),
                            },
                            ShowOn {
                                tile: "square310x310Logo".into(),
                            },
                            ShowOn {
                                tile: "wide310x150Logo".into(),
                            },
                        ]
                        .into(),
                    },
                    splash_screen: SplashScreen {
                        image: "Images\\SplashScreen.png".into(),
                    },
                    lock_screen: LockScreen {
                        badge_logo: "Images\\BadgeLogo.png".into(),
                        notification: "badge".into(),
                    },
                },
            }]
            .into(),
            ..Default::default()
        };
        let xml = quick_xml::se::to_string(&manifest).unwrap();
        println!("{}", xml);
    }
}
