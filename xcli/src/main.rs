use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::Command;
use xcli::config::Config;
use xcli::devices::Device;
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
    if has_dart_code {
        // download flutter engine
        // build assets + dart
        let mut cmd = Command::new("flutter");
        cmd.arg("build");
        let ftarget = match target {
            "x86_64-unknown-linux-gnu" => "linux",
            "aarch64-linux-android" => "apk",
            _ => anyhow::bail!("unsupported target"),
        };
        cmd.arg(ftarget);
        if opt == Opt::Debug {
            cmd.arg("--debug");
        }
        if !cmd.status()?.success() {
            anyhow::bail!("flutter build failed");
        }
    }

    let build_dir = if has_dart_code {
        Path::new("build")
    } else {
        Path::new("target")
    };
    let out_dir = build_dir.join(opt.to_string());
    std::fs::create_dir_all(&out_dir)?;

    let path = match format {
        Format::Appimage => {
            let out = out_dir.join(format!("{}-x86_64.AppImage", &config.name));
            // TODO:
            let build_dir = Path::new("build")
                .join("linux")
                .join("x64")
                .join(opt.to_string());
            let builder = xappimage::AppImageBuilder::new(&build_dir, &out, config.name.clone())?;
            builder.add_directory(&build_dir.join("bundle"), None)?;
            builder.add_apprun()?;
            builder.add_desktop()?;
            if let Some(icon) = config.icon(Format::Appimage) {
                builder.add_icon(icon)?;
            }
            builder.sign(signer)?;
            out
        }
        Format::Apk => {
            use xapk::{Target, VersionCode};

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
            let mut apk = xapk::Apk::new(out.clone())?;
            apk.add_res(manifest.clone(), config.icon(Format::Apk), &android_jar)?;

            // TODO: build assets
            let intermediates = Path::new("build").join("app").join("intermediates");
            let assets = intermediates
                .join("merged_assets")
                .join(opt.to_string())
                .join("out");
            apk.add_assets(&assets)?;

            // TODO: fetch native libs
            let lib = intermediates
                .join("merged_native_libs")
                .join(opt.to_string())
                .join("out")
                .join("lib")
                .join(target.android_abi())
                .join("libflutter.so");
            apk.add_lib(target, &lib)?;

            // TODO: build classes.dex
            let dex = if opt == Opt::Debug {
                "mergeDexDebug"
            } else {
                "minifyReleaseWithR8"
            };
            let classes = intermediates
                .join("dex")
                .join(opt.to_string())
                .join(dex)
                .join("classes.dex");
            apk.add_dex(&classes)?;

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
