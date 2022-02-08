use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use xapk::Target;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sdk {
    sdk_path: PathBuf,
    platforms: Vec<u32>,
    build_tools_version: String,
}

impl Sdk {
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

    pub fn path(&self) -> &Path {
        &self.sdk_path
    }

    pub fn platforms(&self) -> &[u32] {
        &self.platforms
    }

    pub fn build_tools_version(&self) -> &str {
        &self.build_tools_version
    }

    pub fn build_tool(&self, tool: &str) -> Result<Command> {
        let path = self
            .sdk_path
            .join("build-tools")
            .join(&self.build_tools_version)
            .join(tool);
        if !path.exists() {
            anyhow::bail!("build tool {} not found.", tool);
        }
        Ok(Command::new(dunce::canonicalize(path)?))
    }

    pub fn platform_tool(&self, tool: &str) -> Result<Command> {
        let path = self.sdk_path.join("platform-tools").join(tool);
        if !path.exists() {
            anyhow::bail!("platform tool {} not found.", tool);
        }
        Ok(Command::new(dunce::canonicalize(path)?))
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ndk {
    ndk_path: PathBuf,
    build_tag: u32,
    platforms: Vec<u32>,
    sdk: Sdk,
}

impl Ndk {
    pub fn from_env() -> Result<Self> {
        let sdk = Sdk::from_env()?;
        let ndk_path = {
            let ndk_path = std::env::var("ANDROID_NDK_ROOT")
                .ok()
                .or_else(|| std::env::var("ANDROID_NDK_PATH").ok())
                .or_else(|| std::env::var("ANDROID_NDK_HOME").ok())
                .or_else(|| std::env::var("NDK_HOME").ok());

            // default ndk installation path
            if ndk_path.is_none() && sdk.path().join("ndk-bundle").exists() {
                sdk.path().join("ndk-bundle")
            } else {
                PathBuf::from(ndk_path.ok_or_else(|| anyhow::anyhow!("ndk not found"))?)
            }
        };

        let build_tag = std::fs::read_to_string(ndk_path.join("source.properties"))
            .expect("Failed to read source.properties");

        let build_tag = build_tag
            .split('\n')
            .find_map(|line| {
                let (key, value) = line
                    .split_once('=')
                    .expect("Failed to parse `key = value` from source.properties");
                if key.trim() == "Pkg.Revision" {
                    // AOSP writes a constantly-incrementing build version to the patch field.
                    // This number is incrementing across NDK releases.
                    let mut parts = value.trim().split('.');
                    let _major = parts.next().unwrap();
                    let _minor = parts.next().unwrap();
                    let patch = parts.next().unwrap();
                    // Can have an optional `XXX-beta1`
                    let patch = patch.split_once('-').map_or(patch, |(patch, _beta)| patch);
                    Some(patch.parse().expect("Failed to parse patch field"))
                } else {
                    None
                }
            })
            .expect("No `Pkg.Revision` in source.properties");

        let ndk_platforms = std::fs::read_to_string(ndk_path.join("build/core/platforms.mk"))?;
        let ndk_platforms = ndk_platforms
            .split('\n')
            .map(|s| s.split_once(" := ").unwrap())
            .collect::<HashMap<_, _>>();

        let min_platform_level = ndk_platforms["NDK_MIN_PLATFORM_LEVEL"]
            .parse::<u32>()
            .unwrap();
        let max_platform_level = ndk_platforms["NDK_MAX_PLATFORM_LEVEL"]
            .parse::<u32>()
            .unwrap();

        let platforms = sdk
            .platforms()
            .iter()
            .filter(|level| (min_platform_level..=max_platform_level).contains(level))
            .copied()
            .collect();

        Ok(Self {
            sdk,
            ndk_path,
            build_tag,
            platforms,
        })
    }

    pub fn sdk(&self) -> &Sdk {
        &self.sdk
    }

    pub fn path(&self) -> &Path {
        &self.ndk_path
    }

    pub fn build_tag(&self) -> u32 {
        self.build_tag
    }

    pub fn platforms(&self) -> &[u32] {
        &self.platforms
    }

    pub fn toolchain_dir(&self) -> Result<PathBuf> {
        let host_os = std::env::var("HOST").ok();
        let host_contains = |s| host_os.as_ref().map(|h| h.contains(s)).unwrap_or(false);

        let arch = if host_contains("linux") {
            "linux"
        } else if host_contains("macos") {
            "darwin"
        } else if host_contains("windows") {
            "windows"
        } else if cfg!(target_os = "linux") {
            "linux"
        } else if cfg!(target_os = "macos") {
            "darwin"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else {
            match host_os {
                Some(host_os) => anyhow::bail!("unsupported host {}", host_os),
                _ => anyhow::bail!("unsupported target"),
            }
        };

        let mut toolchain_dir = self
            .ndk_path
            .join("toolchains")
            .join("llvm")
            .join("prebuilt")
            .join(format!("{}-x86_64", arch));
        if !toolchain_dir.exists() {
            toolchain_dir.set_file_name(arch);
        }
        if !toolchain_dir.exists() {
            anyhow::bail!("failed to find toolchain dir {}", toolchain_dir.display());
        }
        Ok(toolchain_dir)
    }

    pub fn clang(&self, target: Target, platform: u32) -> Result<(PathBuf, PathBuf)> {
        #[cfg(target_os = "windows")]
        let ext = ".cmd";
        #[cfg(not(target_os = "windows"))]
        let ext = "";

        let bin_name = format!("{}{}-clang", target.ndk_llvm_triple(), platform);
        let bin_path = self.toolchain_dir()?.join("bin");

        let clang = bin_path.join(format!("{}{}", &bin_name, ext));
        if !clang.exists() {
            anyhow::bail!("failed to find clang {}", clang.display());
        }

        let clang_pp = bin_path.join(format!("{}++{}", &bin_name, ext));
        if !clang_pp.exists() {
            anyhow::bail!("failed to find clang++ {}", clang_pp.display());
        }

        Ok((clang, clang_pp))
    }

    pub fn toolchain_bin(&self, name: &str, target: Target) -> Result<PathBuf> {
        #[cfg(target_os = "windows")]
        let ext = ".exe";
        #[cfg(not(target_os = "windows"))]
        let ext = "";

        let toolchain_path = self.toolchain_dir()?.join("bin");

        // Since r21 (https://github.com/android/ndk/wiki/Changelog-r21) LLVM binutils are included _for testing_;
        // Since r22 (https://github.com/android/ndk/wiki/Changelog-r22) GNU binutils are deprecated in favour of LLVM's;
        // Since r23 (https://github.com/android/ndk/wiki/Changelog-r23) GNU binutils have been removed.
        // To maintain stability with the current ndk-build crate release, prefer GNU binutils for
        // as long as it is provided by the NDK instead of trying to use llvm-* from r21 onwards.
        let gnu_bin = format!("{}-{}{}", target.ndk_triple(), name, ext);
        let gnu_path = toolchain_path.join(&gnu_bin);
        if gnu_path.exists() {
            Ok(gnu_path)
        } else {
            let llvm_bin = format!("llvm-{}{}", name, ext);
            let llvm_path = toolchain_path.join(&llvm_bin);
            llvm_path.exists().then(|| llvm_path).ok_or_else(|| {
                anyhow::anyhow!(
                    "toolchain binary {} or {} not found in {}",
                    gnu_bin,
                    llvm_bin,
                    toolchain_path.display(),
                )
            })
        }
    }

    pub fn sysroot_lib_dir(&self, target: Target) -> Result<PathBuf> {
        let sysroot_lib_dir = self
            .toolchain_dir()?
            .join("sysroot")
            .join("usr")
            .join("lib")
            .join(target.ndk_triple());
        if !sysroot_lib_dir.exists() {
            anyhow::bail!("sysroot lib dir {} not found", sysroot_lib_dir.display());
        }
        Ok(sysroot_lib_dir)
    }

    pub fn sysroot_platform_lib_dir(
        &self,
        target: Target,
        min_sdk_version: u32,
    ) -> Result<PathBuf> {
        let sysroot_lib_dir = self.sysroot_lib_dir(target)?;

        // Look for a platform <= min_sdk_version
        let mut tmp_platform = min_sdk_version;
        while tmp_platform > 1 {
            let path = sysroot_lib_dir.join(tmp_platform.to_string());
            if path.exists() {
                return Ok(path);
            }
            tmp_platform += 1;
        }

        // Look for the minimum API level supported by the NDK
        let mut tmp_platform = min_sdk_version;
        while tmp_platform < 100 {
            let path = sysroot_lib_dir.join(tmp_platform.to_string());
            if path.exists() {
                return Ok(path);
            }
            tmp_platform += 1;
        }
        anyhow::bail!("no platform >= {} found", min_sdk_version);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn test_detect() {
        let ndk = Ndk::from_env().unwrap();
        assert_eq!(ndk.sdk().build_tools_version(), "29.0.2");
        assert_eq!(ndk.platforms(), &[29, 28]);
    }
}
