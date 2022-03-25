use crate::download::{DownloadManager, WorkItem};
use crate::flutter::android::FlutterEngine;
use crate::{Arch, CompileTarget, Opt, Platform};
use anyhow::Result;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use zip::ZipArchive;

impl<'a> DownloadManager<'a> {
    pub fn flutter_engine(&mut self, target: CompileTarget) -> Result<PathBuf> {
        let flutter = self.flutter().unwrap();
        let engine_version = flutter.engine_version()?;
        let engine_dir = flutter.engine_dir(target)?;
        let mut artifacts = Vec::with_capacity(4);
        if target.platform() == Platform::host()?
            && target.arch() == Arch::host()?
            && target.opt() == Opt::Debug
        {
            artifacts.push("sky_engine.zip".to_string());
            artifacts.push("flutter_patched_sdk.zip".to_string());
            artifacts.push("flutter_patched_sdk_product.zip".to_string());
            let platform = if target.platform() == Platform::Macos {
                "darwin".to_string()
            } else {
                target.platform().to_string()
            };
            artifacts.push(format!("{}-{}/artifacts.zip", &platform, target.arch()));
            artifacts.push(format!("dart-sdk-{}-{}.zip", &platform, target.arch(),));
        }
        match (target.platform(), target.arch(), target.opt()) {
            (Platform::Linux, arch, Opt::Debug) => {
                artifacts.push(format!(
                    "linux-{arch}/linux-{arch}-flutter-gtk.zip",
                    arch = arch
                ));
            }
            (Platform::Linux, arch, Opt::Release) => {
                artifacts.push(format!(
                    "linux-{arch}-release/linux-{arch}-flutter-gtk.zip",
                    arch = arch
                ));
            }
            (Platform::Macos, arch, Opt::Debug) => {
                artifacts.push(format!("darwin-{}/FlutterMacOS.framework.zip", arch));
            }
            (Platform::Macos, arch, Opt::Release) => {
                artifacts.push(format!("darwin-{}/FlutterMacOS.framework.zip", arch));
            }
            (Platform::Windows, arch, Opt::Debug) => {
                artifacts.push(format!(
                    "windows-{arch}/windows-{arch}-flutter.zip",
                    arch = arch
                ));
            }
            (Platform::Windows, arch, Opt::Release) => {
                artifacts.push(format!(
                    "windows-{arch}-release/windows-{arch}-flutter.zip",
                    arch = arch
                ));
            }
            (Platform::Android, arch, opt) => {
                let abi = target.android_abi();
                let flutter_engine = FlutterEngine::new(abi, opt, &engine_version);
                let flutter_jar = super::android::maven(&engine_dir)?
                    .package(&flutter_engine.package(), &flutter_engine.version())?;
                let mut zip = ZipArchive::new(BufReader::new(File::open(flutter_jar)?))?;
                let mut f = zip.by_name(&format!("lib/{}/libflutter.so", abi.android_abi()))?;
                std::io::copy(&mut f, &mut File::create(engine_dir.join("libflutter.so"))?)?;
                if opt == Opt::Release {
                    let host = Platform::host()?;
                    let platform = if host == Platform::Macos {
                        "darwin".to_string()
                    } else {
                        host.to_string()
                    };
                    artifacts.push(format!(
                        "android-{}-release/{}-{}.zip",
                        arch,
                        &platform,
                        Arch::host()?
                    ));
                }
            }
            (Platform::Ios, _, Opt::Debug) => {
                artifacts.push("ios/artifacts.zip".to_string());
            }
            (Platform::Ios, _, Opt::Release) => {
                artifacts.push("ios-release/artifacts.zip".to_string());
            }
        }
        for artifact in &artifacts {
            let url = format!(
                "https://storage.googleapis.com/flutter_infra_release/flutter/{}/{}",
                &engine_version, artifact
            );
            let file_name = Path::new(artifact).file_name().unwrap().to_str().unwrap();
            self.download(WorkItem::new(engine_dir.join(file_name), url));
        }
        Ok(engine_dir)
    }

    pub fn material_fonts(&mut self) -> Result<PathBuf> {
        let flutter = self.flutter().unwrap();
        let version = flutter.material_fonts_version()?;
        let output = flutter.material_fonts()?;
        let url = format!(
            "https://storage.googleapis.com/flutter_infra_release/flutter/fonts/{}/fonts.zip",
            version,
        );
        self.download(WorkItem::new(output.clone(), url));
        Ok(output)
    }
}
