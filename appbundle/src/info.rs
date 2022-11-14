use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InfoPlist {
    /// A user-visible short name for the bundle.
    #[serde(rename(serialize = "CFBundleName"))]
    pub name: Option<String>,
    /// The user-visible name for the bundle, used by Siri and visible
    /// on the iOS Home screen.
    #[serde(rename(serialize = "CFBundleDisplayName"))]
    pub display_name: Option<String>,
    /// A replacement for the app name in text-to-speech operations.
    #[serde(rename(serialize = "CFBundleSpokenName"))]
    pub spoken_name: Option<String>,

    /// The version of the build that identifies an iteration of the
    /// bundle.
    #[serde(rename(serialize = "CFBundleVersion"))]
    pub version: Option<String>,
    /// The release or version number of the bundle.
    #[serde(rename(serialize = "CFBundleShortVersionString"))]
    pub short_version: Option<String>,
    /// The current version of the Information Property List structure.
    #[serde(rename(serialize = "CFBundleInfoDictionaryVersion"))]
    pub info_dictionary_version: Option<String>,
    /// A human-readable copyright notice for the bundle.
    #[serde(rename(serialize = "NSHumanReadableCopyright"))]
    pub copyright: Option<String>,

    /// A unique identifier for a bundle.
    #[serde(rename(serialize = "CFBundleIdentifier"))]
    pub bundle_identifier: Option<String>,
    /// The category that best describes your app for the App Store.
    #[serde(rename(serialize = "LSApplicationCategoryType"))]
    pub application_category_type: Option<String>,

    /// The minimum version of the operating system required for
    /// the app to run in macOS.
    #[serde(rename(serialize = "LSMinimumSystemVersion"))]
    pub minimum_system_version: Option<String>,
    /// The minimum version of the operating system required for
    /// the app to run in iOS, iPadOS, tvOS, and watchOS.
    #[serde(rename(serialize = "MinimumOSVersion"))]
    pub minimum_os_version: Option<String>,
    /// A boolean value indicating whether the app must run in iOS.
    #[serde(rename(serialize = "LSRequiresIPhoneOS"))]
    pub requires_ios: Option<bool>,

    /// The default language and region for the bundle, as a
    /// language ID.
    #[serde(rename(serialize = "CFBundleDevelopmentRegion"))]
    pub development_region: Option<String>,

    /// The entry point of the bundle.
    #[serde(rename(serialize = "CFBundleExecutable"))]
    pub executable: Option<String>,
    /// The icon file of the bundle.
    #[serde(rename(serialize = "CFBundleIconFile"))]
    pub icon_file: Option<String>,
    /// The icon name of the bundle.
    #[serde(rename(serialize = "CFBundleIconName"))]
    pub icon_name: Option<String>,
    /// The icon files of the bundle.
    #[serde(rename(serialize = "CFBundleIconFiles"))]
    #[serde(default)]
    pub icon_files: Vec<String>,

    #[serde(rename(serialize = "UILaunchStoryboardName"))]
    pub storyboard_name: Option<String>,

    #[serde(rename(serialize = "NSCameraUsageDescription"))]
    pub camera_usage_description: Option<String>,
}
