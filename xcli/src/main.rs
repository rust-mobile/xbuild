use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;
use xapk::zip::read::ZipArchive;
use xapk::{Apk, Target, VersionCode};
use xappimage::AppImage;
use xcli::devices::Device;
use xcli::maven::Dependency;
use xcli::{Arch, BuildArgs, BuildEnv, BuildTarget, CompileTarget, Format, Opt, Platform};
use xcommon::Signer;

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
        if xcommon::stamp_file(
            &flutter.engine_version_path()?,
            &platform_dir.join("engine.version.stamp"),
        )? {
            println!("precaching flutter engine");
            flutter.precache(env.target().platform())?;
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

    if env.has_rust_code() {
        for target in env.target().compile_targets() {
            println!("building rust for {}", target);
            env.cargo(target)?.build()?;
        }
    }

    let out = match env.target().format() {
        Format::Appimage => {
            println!("building appimage");
            let target = env.target().compile_targets().next().unwrap();
            let arch_dir = platform_dir.join(target.arch().to_string());

            let appimage = AppImage::new(&arch_dir, env.name().to_string())?;
            appimage.add_apprun()?;
            appimage.add_desktop()?;
            if let Some(icon) = env.icon() {
                appimage.add_icon(icon)?;
            }

            if let Some(flutter) = env.flutter() {
                let debug_engine_dir = flutter.engine_dir(CompileTarget::new(
                    target.platform(),
                    target.arch(),
                    Opt::Debug,
                ))?;
                let engine_dir = flutter.engine_dir(target)?;
                appimage.add_file(
                    &debug_engine_dir.join("icudtl.dat"),
                    &Path::new("data").join("icudtl.dat"),
                )?;
                appimage.add_file(
                    &engine_dir.join("libflutter_linux_gtk.so"),
                    &Path::new("lib").join("libflutter_linux_gtk.so"),
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
                let bin = Path::new("target")
                    .join(target.opt().to_string())
                    .join(env.name());
                appimage.add_file(&bin, Path::new(env.name()))?;
            }

            if target.opt() == Opt::Release {
                let out = arch_dir.join(format!("{}.AppImage", env.name()));
                appimage.build(&out, env.target().signer().cloned())?;
                out
            } else {
                appimage.appdir().join("AppRun")
            }
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

/*
            let sdk = AndroidSdk::from_env()?;
            let target = Target::from_rust_triple(target)?;

            let manifest = &mut config.apk.manifest;
            let version = manifest
                .version_name
                .get_or_insert_with(|| config.version.clone());
            let version_code = VersionCode::from_semver(version)?.to_code(1);
            manifest.version_code.get_or_insert(version_code);
            let target_sdk = *manifest
                .sdk
                .target_sdk_version
                .get_or_insert_with(|| sdk.default_target_platform());

            let android_jar = sdk.android_jar(target_sdk)?;
            let out = out_dir.join(format!("{}-aarch64.apk", &config.name));
            let mut apk = Apk::new(out.clone())?;
            apk.add_res(manifest.clone(), config.icon(Format::Apk), &android_jar)?;

            if has_dart_code {
                let build_dir = out_dir.join("android");
                let flutter = Flutter::from_env()?;
                let engine_version = flutter.engine_version()?;
                let mvn = Maven::new(build_dir.join("maven"))?;
                let flutter_embedding = Dependency::flutter_embedding(opt, &engine_version);
                let deps = mvn.resolve(flutter_embedding)?;

                // build GeneratedPluginRegistrant
                let plugins = build_dir.join("GeneratedPluginRegistrant.java");
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
                let java = build_dir.join("java");
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
                    .arg(&build_dir)
                    .status()?;
                if !status.success() {
                    anyhow::bail!("d8 exited with nonzero exit code.");
                }
                apk.add_dex(&build_dir.join("classes.dex"))?;

                // add libflutter.so
                let flutter_engine = Dependency::flutter_engine(target, opt, &engine_version);
                let flutter_jar = mvn.package(&flutter_engine)?;
                let mut zip = ZipArchive::new(BufReader::new(File::open(flutter_jar)?))?;
                let f = zip.by_name(&format!("lib/{}/libflutter.so", target.android_abi()))?;
                apk.raw_copy_file(f)?;

                // build assets
                let assets = build_dir.join("assets");
                flutter.copy_flutter_bundle(
                    &assets.join("flutter_assets"),
                    &build_dir.join("flutter_build.d"),
                    opt,
                    Platform::Android,
                    Arch::Arm64,
                )?;
                apk.add_assets(&assets)?;
            }
            apk.finish(signer)?;
            out
*/
