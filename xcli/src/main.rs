use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;
use xapk::zip::read::ZipArchive;
use xapk::{Apk, Target, VersionCode};
use xappimage::AppImage;
use xcli::android::AndroidSdk;
use xcli::config::Config;
use xcli::devices::Device;
use xcli::flutter::Flutter;
use xcli::maven::{Dependency, Maven};
use xcli::{Arch, BuildArgs, BuildTarget, CompileTarget, Format, Opt, Platform};
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
            Self::Build { args } => build(args.build_target()?, false)?,
            Self::Run { args } => build(args.build_target()?, true)?,
        }
        Ok(())
    }
}

fn build(target: BuildTarget, run: bool) -> Result<()> {
    let _has_rust_code = Path::new("Cargo.toml").exists();
    let has_dart_code = Path::new("pubspec.yaml").exists();
    let build_dir = Path::new("target").join("x");
    let opt_dir = build_dir.join(target.opt().to_string());
    let platform_dir = opt_dir.join(target.platform().to_string());
    std::fs::create_dir_all(&platform_dir)?;
    let config = if has_dart_code {
        Config::parse("pubspec.yaml")?
    } else {
        Config::parse("Cargo.toml")?
    };
    let icon = config.icon(target.format());
    let target_file = config.target_file(target.platform());
    // run flutter pub get
    if has_dart_code
        && (!Path::new(".dart_tool").join("package_config.json").exists()
            || xcommon::stamp_file(Path::new("pubspec.yaml"), &build_dir.join("pubspec.stamp"))?)
    {
        let status = Command::new("flutter").arg("pub").arg("get").status()?;
        if !status.success() {
            anyhow::bail!("flutter pub get exited with status {:?}", status);
        }
    }
    // build final artefact
    let out = match target.format() {
        Format::Appimage => {
            let compile_targets = target.compile_targets().collect::<Vec<_>>();
            if compile_targets.len() != 1 {
                anyhow::bail!("expected one compile target for appimage");
            }
            let compile_target = compile_targets[0];
            let arch_dir = platform_dir.join(compile_target.arch().to_string());
            let appimage = AppImage::new(&arch_dir, config.name.clone())?;
            if has_dart_code {
                let flutter = Flutter::from_env()?;
                let debug_engine_dir = flutter.engine_dir(CompileTarget::new(
                    compile_target.platform(),
                    compile_target.arch(),
                    Opt::Debug,
                ))?;
                let engine_dir = flutter.engine_dir(compile_target)?;
                appimage.add_file(
                    &debug_engine_dir.join("icudtl.dat"),
                    &Path::new("data").join("icudtl.dat"),
                )?;
                appimage.add_file(
                    &engine_dir.join("libflutter_linux_gtk.so"),
                    &Path::new("lib").join("libflutter_linux_gtk.so"),
                )?;
                // assemble flutter bundle
                flutter.assemble(
                    &appimage.appdir().join("data").join("flutter_assets"),
                    &arch_dir.join("flutter_build.d"),
                    compile_target,
                )?;
                let kernel_blob = arch_dir.join("kernel_blob.bin");
                flutter.kernel_blob_bin(
                    &target_file,
                    &kernel_blob,
                    &arch_dir.join("kernel_blob.bin.d"),
                    compile_target.opt(),
                )?;
                match compile_target.opt() {
                    Opt::Debug => {
                        appimage.add_file(
                            &kernel_blob,
                            &Path::new("data")
                                .join("flutter_assets")
                                .join("kernel_blob.bin"),
                        )?;
                    }
                    Opt::Release => {
                        let aot_elf = arch_dir.join("libapp.so");
                        flutter.aot_snapshot(&kernel_blob, &aot_elf, compile_target)?;
                        appimage.add_file(&aot_elf, &Path::new("lib").join("libapp.so"))?;
                    }
                }
            }
            build_rust(compile_target, target.is_host())?;
            let bin = Path::new("target")
                .join(target.opt().to_string())
                .join(&config.name);
            appimage.add_file(&bin, Path::new(&config.name))?;

            appimage.add_apprun()?;
            appimage.add_desktop()?;
            if let Some(icon) = icon {
                appimage.add_icon(icon)?;
            }
            if target.opt() == Opt::Release {
                let out = arch_dir.join(format!("{}.AppImage", &config.name));
                appimage.build(&out, target.signer().cloned())?;
                out
            } else {
                appimage.appdir().join("AppRun")
            }
        }
        f => unimplemented!("{:?}", f),
    };
    println!("built {}", out.display());
    // maybe run
    if run {
        if let Some(device) = target.device() {
            device.run(&out, &config, has_dart_code)?;
        } else {
            anyhow::bail!("no device specified");
        }
    }
    Ok(())
}

fn build_rust(target: CompileTarget, is_host: bool) -> Result<()> {
    // TODO: cleanup and generalize
    let mut cmd = Command::new("cargo");
    cmd.env(
        "RUSTFLAGS",
        format!(
            "-Clink-arg=-L/home/dvc/cloudpeer/helloworld/target/x/{}/{}/{}/helloworld.AppDir/lib",
            target.opt(),
            target.platform(),
            target.arch(),
        ),
    );
    cmd.arg("build");
    if target.opt() == Opt::Release {
        cmd.arg("--release");
    }
    if !is_host {
        cmd.arg("--target");
        cmd.arg(target.rust_triple()?);
    }
    // configure tools and linkers for sdk
    if !cmd.status()?.success() {
        anyhow::bail!("cargo build failed");
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
