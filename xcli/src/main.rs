use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;
use xapk::zip::read::ZipArchive;
use xapk::{Apk, Target, VersionCode};
use xappimage::AppImageBuilder;
use xcli::config::Config;
use xcli::devices::Device;
use xcli::sdk::flutter::{Arch, Flutter, Platform};
use xcli::sdk::maven::{Dependency, Maven};
use xcli::{Format, Opt};
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

#[derive(Parser)]
struct BuildArgs {
    #[clap(long)]
    release: bool,
    #[clap(long)]
    device: Option<Device>,
    #[clap(long)]
    key: Option<PathBuf>,
    #[clap(long)]
    cert: Option<PathBuf>,
}

impl BuildArgs {
    pub fn device(&self) -> Device {
        if let Some(device) = &self.device {
            device.clone()
        } else {
            Device::host()
        }
    }

    pub fn opt(&self) -> Opt {
        if self.release {
            Opt::Release
        } else {
            Opt::Debug
        }
    }

    pub fn signer(&self) -> Result<Option<Signer>> {
        if let (Some(key), Some(cert)) = (self.key.as_ref(), self.cert.as_ref()) {
            let key = std::fs::read_to_string(key)?;
            let cert = std::fs::read_to_string(cert)?;
            Ok(Some(Signer::new(&key, &cert)?))
        } else {
            Ok(None)
        }
    }
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
    pub fn run(&self) -> Result<()> {
        match self {
            Self::Devices => {
                for device in Device::list()? {
                    println!(
                        "{:20}{:20}{:30}{}",
                        device.to_string(),
                        device.name()?,
                        device.target()?,
                        device.platform()?
                    );
                }
            }
            Self::Build { args } => build(args, false)?,
            Self::Run { args } => build(args, true)?,
        }
        Ok(())
    }
}

fn build(args: &BuildArgs, run: bool) -> Result<()> {
    let opt = args.opt();
    let signer = args.signer()?;
    let device = args.device();
    let target = device.target()?;
    let format = Format::from_target(&target)?;
    let has_rust_code = Path::new("Cargo.toml").exists();
    let has_dart_code = Path::new("pubspec.yaml").exists();
    let mut config = if has_dart_code {
        Config::parse("pubspec.yaml")?
    } else {
        Config::parse("Cargo.toml")?
    };
    if has_rust_code {
        let mut cmd = Command::new("cargo");
        // TODO:
        cmd.env(
            "RUSTFLAGS",
            "-Clink-arg=-L/home/dvc/cloudpeer/helloworld/build/debug/linux/helloworld.AppDir/lib",
        );
        cmd.arg("build");
        if opt == Opt::Release {
            cmd.arg("--release");
        }
        if !device.is_host() {
            cmd.arg("--target");
            cmd.arg(target);
        }
        // configure tools and linkers for sdk
        if !cmd.status()?.success() {
            anyhow::bail!("cargo build failed");
        }
    }
    let build_dir = if has_dart_code {
        Path::new("build")
    } else {
        Path::new("target")
    };
    let out_dir = build_dir.join(opt.to_string());
    std::fs::create_dir_all(&out_dir)?;

    let pubspec_modified = if has_dart_code {
        let stamp = build_dir.join("pubspec.stamp");
        let exists = stamp.exists();
        let stamp_time = File::create(stamp)?.metadata()?.modified()?;
        let pubspec_time = File::open("pubspec.yaml")?.metadata()?.modified()?;
        !exists || pubspec_time > stamp_time
    } else {
        false
    };
    if pubspec_modified {
        let status = Command::new("flutter").arg("pub").arg("get").status()?;
        if !status.success() {
            anyhow::bail!("flutter pub get exited with status {:?}", status);
        }
    }
    let path = match format {
        Format::Appimage => {
            let flutter = Flutter::from_env()?;
            let build_dir = out_dir.join("linux");
            let engine_dir = flutter.engine_dir(Platform::Linux, Arch::X64, opt)?;
            let out = out_dir.join(format!("{}-x86_64.AppImage", &config.name));

            let appimage = AppImageBuilder::new(&build_dir, &out, config.name.clone())?;
            flutter.copy_flutter_bundle(
                &appimage.appdir().join("data").join("flutter_assets"),
                &build_dir.join("flutter_build.d"),
                opt,
                Platform::Linux,
                Arch::X64,
            )?;
            appimage.add_file(
                &engine_dir.join("icudtl.dat"),
                &Path::new("data").join("icudtl.dat"),
            )?;
            appimage.add_file(
                &engine_dir.join("libflutter_linux_gtk.so"),
                &Path::new("lib").join("libflutter_linux_gtk.so"),
            )?;
            // TODO: build real binary
            appimage.add_file(
                //&Path::new("linux").join(&config.name),
                &Path::new("target").join("debug").join("helloworld"),
                Path::new(&config.name),
            )?;
            appimage.add_apprun()?;
            appimage.add_desktop()?;
            if let Some(icon) = config.icon(Format::Appimage) {
                appimage.add_icon(icon)?;
            }
            appimage.sign(signer)?;
            out
        }
        Format::Apk => {
            let sdk = xcli::sdk::android::Sdk::from_env()?;
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
        }
        _ => unimplemented!("{:?}", format),
    };
    if run {
        device.run(&path, &config, has_dart_code)?;
    }
    Ok(())
}
