use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::process::Command;
use xapk::zip::read::ZipArchive;
use xapk::Apk;
use xappimage::AppImage;
use xcli::devices::Device;
use xcli::flutter::Flutter;
use xcli::maven::Dependency;
use xcli::{BuildArgs, BuildEnv, Format, Opt, Platform};
use xcommon::ZipFileOptions;
use xmsix::Msix;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(subcommand)]
    command: Commands,
}

fn main() -> Result<()> {
    let args = Args::parse();
    args.command.run()
}

#[derive(Subcommand)]
enum Commands {
    Devices,
    Build {
        #[clap(flatten)]
        args: BuildArgs,
    },
    Run {
        #[clap(flatten)]
        args: BuildArgs,
    },
}

impl Commands {
    pub fn run(self) -> Result<()> {
        match self {
            Self::Devices => {
                for device in Device::list()? {
                    println!(
                        "{:20}{:20}{:20}{}",
                        device.to_string(),
                        device.name()?,
                        format!("{} {}", device.platform()?, device.arch()?),
                        device.details()?,
                    );
                }
            }
            Self::Build { args } => build(args, false)?,
            Self::Run { args } => build(args, true)?,
        }
        Ok(())
    }
}

fn build(args: BuildArgs, run: bool) -> Result<()> {
    let env = BuildEnv::new(args)?;
    let opt_dir = env.build_dir().join(env.target().opt().to_string());
    let platform_dir = opt_dir.join(env.target().platform().to_string());

    if env.target().platform() == Platform::Windows && Platform::host()? != Platform::Windows {
        let windows_sdk = env.build_dir().join("Windows.sdk");
        if !windows_sdk.exists() {
            println!("downloading windows sdk");
            xcli::github::download_tar_zst(
                env.build_dir(),
                "cloudpeers",
                "xcross",
                "v0.1.0+1",
                "Windows.sdk.tar.zst",
            )?;
        }
    }

    if let Some(flutter) = env.flutter() {
        if !Path::new(".dart_tool").join("package_config.json").exists()
            || xcommon::stamp_file(
                Path::new("pubspec.yaml"),
                &env.build_dir().join("pubspec.stamp"),
            )?
        {
            println!("pub get");
            flutter.pub_get()?;
        }
        let engine_version_changed = xcommon::stamp_file(
            &flutter.engine_version_path()?,
            &platform_dir.join("engine.version.stamp"),
        )?;
        if engine_version_changed {
            println!("precaching flutter engine");
            flutter.precache(env.target().platform())?;
        }
        if env.target().platform() == Platform::Android {
            if engine_version_changed || !platform_dir.join("classes.dex").exists() {
                println!("building classes.dex");
                build_classes_dex(&env, &flutter, &platform_dir)?;
            }
        }
        println!("building flutter_assets");
        flutter.build_flutter_assets(
            &env.build_dir().join("flutter_assets"),
            &env.build_dir().join("flutter_assets.d"),
        )?;
        println!("building kernel_blob.bin");
        let kernel_blob = platform_dir.join("kernel_blob.bin");
        flutter.kernel_blob_bin(
            env.target_file(),
            &kernel_blob,
            &platform_dir.join("kernel_blob.bin.d"),
            env.target().opt(),
        )?;
        if env.target().opt() == Opt::Release
            && xcommon::stamp_file(&kernel_blob, &platform_dir.join("kernel_blob.bin.stamp"))?
        {
            for target in env.target().compile_targets() {
                println!("building aot snapshot for {}", target);
                let arch_dir = platform_dir.join(target.arch().to_string());
                std::fs::create_dir_all(&arch_dir)?;
                flutter.aot_snapshot(&kernel_blob, &arch_dir.join("libapp.so"), target)?;
            }
        }
    }

    // TODO: skipping build on android for now
    if env.has_rust_code() && env.target().platform() != Platform::Android {
        for target in env.target().compile_targets() {
            println!("building rust for {}", target);
            env.cargo(target)?.build()?;
        }
    }

    println!("building {}", env.target().format());
    let out = match env.target().format() {
        Format::Appimage => {
            let target = env.target().compile_targets().next().unwrap();
            let arch_dir = platform_dir.join(target.arch().to_string());

            let appimage = AppImage::new(&arch_dir, env.name().to_string())?;
            appimage.add_apprun()?;
            appimage.add_desktop()?;
            if let Some(icon) = env.icon() {
                appimage.add_icon(icon)?;
            }

            if let Some(flutter) = env.flutter() {
                let engine_dir = flutter.engine_dir(target)?;
                appimage.add_file(
                    &flutter.icudtl_dat()?,
                    &Path::new("data").join("icudtl.dat"),
                )?;
                appimage.add_file(
                    &engine_dir.join("libflutter_linux_gtk.so"),
                    &Path::new("lib").join("libflutter_linux_gtk.so"),
                )?;
                appimage.add_directory(
                    &env.build_dir().join("flutter_assets"),
                    &Path::new("data").join("flutter_assets"),
                )?;
                match target.opt() {
                    Opt::Debug => {
                        appimage.add_file(
                            &platform_dir.join("kernel_blob.bin"),
                            &Path::new("data")
                                .join("flutter_assets")
                                .join("kernel_blob.bin"),
                        )?;
                    }
                    Opt::Release => {
                        appimage.add_file(
                            &arch_dir.join("libapp.so"),
                            &Path::new("lib").join("libapp.so"),
                        )?;
                    }
                }
            }

            if env.has_rust_code() {
                appimage.add_file(&env.cargo_artefact(target)?, Path::new(env.name()))?;
            }

            if target.opt() == Opt::Release {
                let out = arch_dir.join(format!("{}.AppImage", env.name()));
                appimage.build(&out, env.target().signer().cloned())?;
                out
            } else {
                appimage.appdir().join("AppRun")
            }
        }
        Format::Apk => {
            let out = platform_dir.join(format!("{}.apk", env.name()));
            let mut apk = Apk::new(out.clone())?;
            apk.add_res(
                env.android_manifest().unwrap().clone(),
                env.icon(),
                &env.android_jar()?,
            )?;
            if let Some(flutter) = env.flutter() {
                // add libflutter.so
                let engine_version = flutter.engine_version()?;
                for target in env.target().compile_targets() {
                    let abi = target.android_abi()?;
                    let flutter_engine =
                        Dependency::flutter_engine(abi, target.opt(), &engine_version);
                    let flutter_jar = env.maven()?.package(&flutter_engine)?;
                    let mut zip = ZipArchive::new(BufReader::new(File::open(flutter_jar)?))?;
                    let f = zip.by_name(&format!("lib/{}/libflutter.so", abi.android_abi()))?;
                    apk.add_zip_file(f)?;
                }
                apk.add_dex(&platform_dir.join("classes.dex"))?;
                apk.add_directory(
                    &env.build_dir().join("flutter_assets"),
                    &Path::new("assets").join("flutter_assets"),
                )?;
                apk.add_file(
                    &flutter.vm_snapshot_data()?,
                    &Path::new("assets")
                        .join("flutter_assets")
                        .join("vm_snapshot_data"),
                    ZipFileOptions::Compressed,
                )?;
                apk.add_file(
                    &flutter.isolate_snapshot_data()?,
                    &Path::new("assets")
                        .join("flutter_assets")
                        .join("isolate_snapshot_data"),
                    ZipFileOptions::Compressed,
                )?;
                match env.target().opt() {
                    Opt::Debug => {
                        apk.add_file(
                            &platform_dir.join("kernel_blob.bin"),
                            &Path::new("assets")
                                .join("flutter_assets")
                                .join("kernel_blob.bin"),
                            ZipFileOptions::Compressed,
                        )?;
                    }
                    Opt::Release => {
                        for target in env.target().compile_targets() {
                            apk.add_lib(
                                target.android_abi()?,
                                &platform_dir
                                    .join(target.arch().to_string())
                                    .join("libapp.so"),
                            )?;
                        }
                    }
                }
            }
            apk.finish(env.target().signer().cloned())?;
            out
        }
        Format::Msix => {
            let target = env.target().compile_targets().next().unwrap();
            let arch_dir = platform_dir.join(target.arch().to_string());
            std::fs::create_dir_all(&arch_dir)?;
            let out = arch_dir.join(format!("{}.msix", env.name()));
            let mut msix = Msix::new(out.clone())?;
            msix.add_manifest(env.appx_manifest().unwrap())?;
            if let Some(icon) = env.icon() {
                msix.add_icon(icon)?;
            }
            // TODO: *.pri

            if let Some(flutter) = env.flutter() {
                let engine_dir = flutter.engine_dir(target)?;
                msix.add_file(
                    &flutter.icudtl_dat()?,
                    &Path::new("data").join("icudtl.dat"),
                    ZipFileOptions::Compressed,
                )?;
                msix.add_file(
                    &engine_dir.join("flutter_windows.dll"),
                    &Path::new("flutter_windows.dll"),
                    ZipFileOptions::Compressed,
                )?;
                msix.add_directory(
                    &env.build_dir().join("flutter_assets"),
                    &Path::new("data").join("flutter_assets"),
                )?;
                match target.opt() {
                    Opt::Debug => {
                        msix.add_file(
                            &platform_dir.join("kernel_blob.bin"),
                            &Path::new("data")
                                .join("flutter_assets")
                                .join("kernel_blob.bin"),
                            ZipFileOptions::Compressed,
                        )?;
                    }
                    Opt::Release => {
                        msix.add_file(
                            &arch_dir.join("libapp.so"),
                            &Path::new("data").join("app.so"),
                            ZipFileOptions::Compressed,
                        )?;
                    }
                }
            }
            if env.has_rust_code() {
                msix.add_file(
                    &env.cargo_artefact(target)?,
                    format!("{}.exe", env.name()).as_ref(),
                    ZipFileOptions::Compressed,
                )?;
            }
            msix.finish(env.target().signer().cloned())?;
            out
        }
        f => unimplemented!("{:?}", f),
    };
    println!("built {}", out.display());

    if run {
        if let Some(device) = env.target().device() {
            device.run(&out, &env, env.has_dart_code())?;
        } else {
            anyhow::bail!("no device specified");
        }
    }
    Ok(())
}

fn build_classes_dex(env: &BuildEnv, flutter: &Flutter, platform_dir: &Path) -> Result<()> {
    let engine_version = flutter.engine_version()?;
    let android_jar = env.android_jar()?;
    let flutter_embedding = Dependency::flutter_embedding(env.target().opt(), &engine_version);
    let deps = env.maven()?.resolve(flutter_embedding)?;

    // build GeneratedPluginRegistrant
    let plugins = platform_dir.join("GeneratedPluginRegistrant.java");
    std::fs::write(
        &plugins,
        include_bytes!("../assets/GeneratedPluginRegistrant.java"),
    )?;
    let classpath = deps
        .iter()
        .chain(std::iter::once(&android_jar))
        .map(|d| d.display().to_string())
        .collect::<Vec<_>>()
        .join(":");
    let java = platform_dir.join("java");
    let status = Command::new("javac")
        .arg("--class-path")
        .arg(classpath)
        .arg(plugins)
        .arg("-d")
        .arg(&java)
        .status()?;
    if !status.success() {
        anyhow::bail!("javac exited with nonzero exit code.");
    }

    // build classes.dex
    let plugins = java
        .join("io")
        .join("flutter")
        .join("plugins")
        .join("GeneratedPluginRegistrant.class");
    let status = Command::new("d8")
        .args(deps)
        .arg(plugins)
        .arg("--lib")
        .arg(android_jar)
        .arg("--output")
        .arg(platform_dir)
        .status()?;
    if !status.success() {
        anyhow::bail!("d8 exited with nonzero exit code.");
    }
    Ok(())
}
