use crate::download::{DownloadManager, WorkItem};
use crate::{Arch, CompileTarget, Opt, Platform};
use anyhow::Result;
use maven::{Maven, Package, Version};
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use xapk::Target;
use zip::ZipArchive;

impl<'a> DownloadManager<'a> {
    fn maven(&'a self) -> Result<Maven<&'a Self>> {
        const GOOGLE: &str = "https://maven.google.com";
        const FLUTTER: &str = "http://download.flutter.io";
        const CENTRAL: &str = "https://repo1.maven.org/maven2";

        let mut maven = Maven::new(self.env().cache_dir().join("maven"), self)?;
        maven.add_repository(GOOGLE);
        maven.add_repository(FLUTTER);
        maven.add_repository(CENTRAL);
        Ok(maven)
    }

    pub fn r8(&self) -> Result<PathBuf> {
        let maven = self.maven()?;
        let package = Package {
            group: "com.android.tools".into(),
            name: "r8".into(),
        };
        let version = Version {
            major: 3,
            minor: 1,
            patch: 51,
            suffix: None,
        };
        maven.package(&package, &version)
    }

    pub fn flutter_embedding(&self) -> Result<Vec<PathBuf>> {
        let maven = self.maven()?;
        let package = Package {
            group: "io.flutter".into(),
            name: format!("flutter_embedding_{}", self.env().target().opt()),
        };
        let version = Version {
            major: 1,
            minor: 0,
            patch: 0,
            suffix: Some(self.env().flutter().unwrap().engine_version()?),
        };
        let deps = maven
            .resolve(package, version)?
            .into_iter()
            .filter(|path| {
                path.extension() == Some("jar".as_ref()) || path.extension() == Some("aar".as_ref())
            })
            .collect::<Vec<_>>();
        Ok(deps)
    }

    pub fn flutter_engine(&self, target: CompileTarget) -> Result<()> {
        let flutter = self.env().flutter().unwrap();
        let engine_version = flutter.engine_version()?;
        let engine_dir = flutter.engine_dir(target)?;
        let mut artifacts = Vec::with_capacity(4);
        if target.platform() == Platform::host()?
            && target.arch() == Arch::host()?
            && target.opt() == Opt::Debug
        {
            artifacts.push(("sky_engine.zip".to_string(), "sky_engine"));
            artifacts.push(("flutter_patched_sdk.zip".to_string(), "flutter_patched_sdk"));
            artifacts.push((
                "flutter_patched_sdk_product.zip".to_string(),
                "flutter_patched_sdk_product",
            ));
            let platform = if target.platform() == Platform::Macos {
                "darwin".to_string()
            } else {
                target.platform().to_string()
            };
            artifacts.push((
                format!("{}-{}/artifacts.zip", &platform, target.arch()),
                "frontend_server.dart.snapshot",
            ));
            artifacts.push((
                format!("dart-sdk-{}-{}.zip", &platform, target.arch()),
                "dart-sdk",
            ));
        }
        match (target.platform(), target.arch(), target.opt()) {
            (Platform::Linux, arch, Opt::Debug) => {
                artifacts.push((
                    format!("linux-{arch}/linux-{arch}-flutter-gtk.zip", arch = arch),
                    "libflutter_linux_gtk.so",
                ));
            }
            (Platform::Linux, arch, Opt::Release) => {
                artifacts.push((
                    format!(
                        "linux-{arch}-release/linux-{arch}-flutter-gtk.zip",
                        arch = arch
                    ),
                    "libflutter_linux_gtk.so",
                ));
            }
            (Platform::Macos, arch, Opt::Debug) => {
                artifacts.push((
                    format!("darwin-{}/FlutterMacOS.framework.zip", arch),
                    "FlutterMacOS.framework",
                ));
            }
            (Platform::Macos, arch, Opt::Release) => {
                artifacts.push((
                    format!("darwin-{}-release/FlutterMacOS.framework.zip", arch),
                    "FlutterMacOS.framework",
                ));
                artifacts.push((
                    format!("darwin-{}-release/artifacts.zip", arch),
                    "gen_snapshot",
                ));
            }
            (Platform::Windows, arch, Opt::Debug) => {
                artifacts.push((
                    format!("windows-{arch}/windows-{arch}-flutter.zip", arch = arch),
                    "flutter_windows.dll",
                ));
            }
            (Platform::Windows, arch, Opt::Release) => {
                artifacts.push((
                    format!(
                        "windows-{arch}-release/windows-{arch}-flutter.zip",
                        arch = arch
                    ),
                    "flutter_windows.dll",
                ));
            }
            (Platform::Android, arch, opt) => {
                let output = engine_dir.join("libflutter.so");
                if !output.exists() {
                    let maven = self.maven()?;
                    let abi = target.android_abi();
                    let pabi = match abi {
                        Target::Arm64V8a => "arm64_v8a",
                        Target::ArmV7a => "armeabi_v7a",
                        Target::X86 => "x86",
                        Target::X86_64 => "x86_64",
                    };
                    let package = Package {
                        group: "io.flutter".into(),
                        name: format!("{}_{}", pabi, target.opt()),
                    };
                    let version = Version {
                        major: 1,
                        minor: 0,
                        patch: 0,
                        suffix: Some(engine_version.clone()),
                    };
                    let flutter_jar = maven.package(&package, &version)?;
                    let mut zip = ZipArchive::new(BufReader::new(File::open(flutter_jar)?))?;
                    let mut f = zip.by_name(&format!("lib/{}/libflutter.so", abi.android_abi()))?;
                    std::fs::create_dir_all(&engine_dir)?;
                    std::io::copy(&mut f, &mut File::create(output)?)?;
                }

                if opt == Opt::Release {
                    let host = Platform::host()?;
                    let platform = if host == Platform::Macos {
                        "darwin".to_string()
                    } else {
                        host.to_string()
                    };
                    artifacts.push((
                        format!(
                            "android-{}-release/{}-{}.zip",
                            arch,
                            &platform,
                            Arch::host()?
                        ),
                        exe!("gen_snapshot"),
                    ));
                }
            }
            (Platform::Ios, _, Opt::Debug) => {
                artifacts.push(("ios/artifacts.zip".to_string(), "Flutter.xcframework"));
            }
            (Platform::Ios, _, Opt::Release) => {
                artifacts.push((
                    "ios-release/artifacts.zip".to_string(),
                    "Flutter.xcframework",
                ));
            }
        }
        for (artifact, output) in &artifacts {
            let url = format!(
                "https://storage.googleapis.com/flutter_infra_release/flutter/{}/{}",
                &engine_version, artifact
            );
            self.fetch(WorkItem::new(engine_dir.join(output), url))?;
        }
        Ok(())
    }

    pub fn material_fonts(&self) -> Result<()> {
        let flutter = self.env().flutter().unwrap();
        let version = flutter.material_fonts_version()?;
        let output = flutter.material_fonts()?;
        let url = format!(
            "https://storage.googleapis.com/flutter_infra_release/flutter/fonts/{}/fonts.zip",
            version,
        );
        self.fetch(WorkItem::new(output.join("MaterialIcons-Regular.otf"), url))
    }
}
