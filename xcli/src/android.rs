use anyhow::{Context, Result};
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AndroidSdk {
    sdk_path: PathBuf,
    platforms: Vec<u32>,
    build_tools_version: String,
}

impl AndroidSdk {
    pub fn from_env() -> Result<Self> {
        let sdk_path = {
            let mut sdk_path = std::env::var("ANDROID_HOME").ok();
            if sdk_path.is_none() {
                sdk_path = std::env::var("ANDROID_SDK_ROOT").ok();
            }
            let sdk_path = sdk_path.ok_or_else(|| anyhow::anyhow!("sdk not found"))?;
            PathBuf::from(sdk_path)
        };
        let platforms_dir = sdk_path.join("platforms");
        let platforms: Vec<u32> = std::fs::read_dir(&platforms_dir)
            .with_context(|| format!("failed to open platforms dir {}", platforms_dir.display()))?
            .filter_map(|path| path.ok())
            .filter(|path| path.path().is_dir())
            .filter_map(|path| path.file_name().into_string().ok())
            .filter_map(|name| {
                name.strip_prefix("android-")
                    .and_then(|api| api.parse::<u32>().ok())
            })
            .collect();
        if platforms.is_empty() {
            anyhow::bail!("no platform found");
        }
        let build_tools_dir = sdk_path.join("build-tools");
        let build_tools_version = std::fs::read_dir(&build_tools_dir)
            .with_context(|| {
                format!(
                    "failed to open build tools dir {}",
                    build_tools_dir.display()
                )
            })?
            .filter_map(|path| path.ok())
            .filter(|path| path.path().is_dir())
            .filter_map(|path| path.file_name().into_string().ok())
            .filter(|name| name.chars().next().unwrap().is_digit(10))
            .max()
            .ok_or_else(|| anyhow::anyhow!("build tools not found"))?;
        Ok(Self {
            sdk_path,
            platforms,
            build_tools_version,
        })
    }

    pub fn platforms(&self) -> &[u32] {
        &self.platforms
    }

    pub fn highest_supported_platform(&self) -> u32 {
        self.platforms().iter().max().cloned().unwrap()
    }

    /// Returns platform `30` as currently [required by Google Play], or lower
    /// when the detected SDK does not support it yet.
    ///
    /// [required by Google Play]: https://developer.android.com/distribute/best-practices/develop/target-sdk
    pub fn default_target_platform(&self) -> u32 {
        self.highest_supported_platform().min(30)
    }

    pub fn platform_dir(&self, platform: u32) -> Result<PathBuf> {
        let dir = self
            .sdk_path
            .join("platforms")
            .join(format!("android-{}", platform));
        if !dir.exists() {
            anyhow::bail!("platform {} not found.", platform);
        }
        Ok(dir)
    }

    pub fn android_jar(&self, platform: u32) -> Result<PathBuf> {
        let android_jar = self.platform_dir(platform)?.join("android.jar");
        if !android_jar.exists() {
            anyhow::bail!("{} not found.", android_jar.display());
        }
        Ok(android_jar)
    }
}
