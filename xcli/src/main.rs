use anyhow::Result;
use clap::{Parser, Subcommand};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;
use xcli::{Config, Format, Mode};
use xcommon::{Signer, ZipFileOptions};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Build {
        #[clap(flatten)]
        build: BuildOptions,
        #[clap(flatten)]
        sign: SignOptions,
    },
    Sign {
        #[clap(flatten)]
        sign: SignOptions,
        file: PathBuf,
    },
    Verify {
        file: PathBuf,
    },
    Run {
        #[clap(flatten)]
        build: BuildOptions,
        #[clap(flatten)]
        sign: SignOptions,
        #[clap(flatten)]
        run: RunOptions,
    },
}

#[derive(Parser, Debug)]
struct BuildOptions {
    #[clap(long)]
    debug: bool,
    #[clap(long)]
    target: Option<String>,
}

#[derive(Parser, Debug)]
struct SignOptions {
    #[clap(long)]
    key: Option<PathBuf>,
    #[clap(long)]
    cert: Option<PathBuf>,
}

impl SignOptions {
    fn signer(&self) -> Result<Option<Signer>> {
        if let (Some(key), Some(cert)) = (self.key.as_ref(), self.cert.as_ref()) {
            let key = std::fs::read_to_string(key)?;
            let cert = std::fs::read_to_string(cert)?;
            Ok(Some(Signer::new(&key, &cert)?))
        } else {
            Ok(None)
        }
    }
}

#[derive(Parser, Debug)]
struct RunOptions {
    #[clap(short, long)]
    device: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Commands::Build { build, sign } => {
            let path = cmd_build_and_sign(build, sign)?;
            println!("built {}", path.display());
        }
        Commands::Sign { sign, file } => {
            cmd_sign(sign, &file)?;
        }
        Commands::Verify { file } => {
            cmd_verify(&file)?;
        }
        Commands::Run { build, sign, run } => {
            let path = cmd_build_and_sign(build, sign)?;
            cmd_run(&path, run)?;
        }
    }
    Ok(())
}

fn cmd_build_and_sign(build: BuildOptions, sign: SignOptions) -> Result<PathBuf> {
    let format = if let Some(triple) = build.target.as_deref() {
        Format::from_target(triple)?
    } else {
        Format::from_target(host_triple()?)?
    };
    let signer = sign.signer()?;
    let (config, mode) = if Path::new("Cargo.toml").exists() {
        (Config::parse("Cargo.toml")?, Mode::Cargo)
    } else if Path::new("pubspec.yaml").exists() {
        (Config::parse("pubspec.yaml")?, Mode::Flutter)
    } else {
        anyhow::bail!("config file not found");
    };
    let opt = if build.debug { "debug" } else { "release" };
    let out_dir = match mode {
        Mode::Cargo => Path::new("target").join(opt),
        Mode::Flutter => Path::new("build").join(opt),
    };
    std::fs::create_dir_all(&out_dir)?;
    match (mode, format) {
        (Mode::Flutter, Format::Appimage) => {
            flutter_build("linux", build.debug)?;
            let out = out_dir.join(format!("{}-x86_64.AppImage", &config.name));
            let build_dir = Path::new("build").join("linux").join("x64").join(opt);
            let builder = xappimage::AppImageBuilder::new(&build_dir, &out, config.name.clone())?;
            builder.add_directory(&build_dir.join("bundle"), None)?;
            builder.add_apprun()?;
            builder.add_desktop()?;
            if let Some(icon) = config.icon(Format::Appimage) {
                builder.add_icon(icon)?;
            }
            builder.sign(signer)?;
            Ok(out)
        }
        (Mode::Flutter, Format::Apk) => {
            flutter_build("apk", build.debug)?;
            let out = out_dir.join(format!("{}-aarch64.apk", &config.name));
            let mut apk = File::create(&out)?;
            let mut builder = xapk::ApkBuilder::new(&mut apk);
            let intermediates = Path::new("build").join("app").join("intermediates");
            let assets = intermediates.join("merged_assets").join(opt).join("out");
            builder.add_directory(&assets, Some(Path::new("assets")))?;
            let libs = intermediates
                .join("merged_native_libs")
                .join(opt)
                .join("out");
            builder.add_directory(&libs, None)?;
            let dex = if build.debug {
                "mergeDexDebug"
            } else {
                "minifyReleaseWithR8"
            };
            let classes = intermediates
                .join("dex")
                .join(opt)
                .join(dex)
                .join("classes.dex");
            builder.add_file(&classes, "classes.dex", ZipFileOptions::Compressed)?;
            let manifest = intermediates
                .join("merged_manifest")
                .join(opt)
                .join("out")
                .join("AndroidManifest.xml");
            builder.add_manifest(&xapk::Xml::from_path(&manifest)?)?;
            builder.sign(signer)?;
            Ok(out)
        }
        f => unimplemented!("{:?}", f),
    }
}

fn cmd_sign(opts: SignOptions, file: &Path) -> Result<()> {
    match Format::from_path(file)? {
        Format::Apk => xapk::sign::sign(file, opts.signer()?)?,
        f => unimplemented!("{:?}", f),
    }
    Ok(())
}

fn cmd_verify(file: &Path) -> Result<()> {
    let certs = match Format::from_path(file)? {
        Format::Apk => xapk::sign::verify(file)?,
        Format::Msix => {
            let signed_data = xmsix::p7x::read_p7x(file)?;
            for signer in &signed_data.signer_infos {
                if let rasn_cms::SignerIdentifier::IssuerAndSerialNumber(isn) = &signer.sid {
                    println!("issuer: {}", display_cert_name(&isn.issuer)?);
                }
            }
            return Ok(());
        }
        f => unimplemented!("{:?}", f),
    };
    for cert in certs {
        println!(
            "subject: {}",
            display_cert_name(&cert.tbs_certificate.subject)?
        );
        println!(
            "issuer: {}",
            display_cert_name(&cert.tbs_certificate.issuer)?
        );
    }
    Ok(())
}

fn cmd_run(file: &Path, opts: RunOptions) -> Result<()> {
    todo!();
}

fn host_triple() -> Result<&'static str> {
    Ok(if cfg!(target_os = "linux") {
        "x86_64-unknown-linux-gnu"
    } else if cfg!(target_os = "macos") {
        "x86_64-apple-darwin"
    } else if cfg!(target_os = "windows") {
        "x86_64-pc-windows-msvc"
    } else {
        anyhow::bail!("unsupported host");
    })
}

fn flutter_build(target: &str, debug: bool) -> Result<()> {
    let mut cmd = Command::new("flutter");
    cmd.arg("build").arg(target);
    if debug {
        cmd.arg("--debug");
    }
    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("failed to run flutter");
    }
    Ok(())
}

fn display_cert_name(name: &rasn_pkix::Name) -> Result<String> {
    use rasn::prelude::Oid;
    let rasn_pkix::Name::RdnSequence(seq) = name;
    let mut attrs = vec![];
    for set in seq {
        for attr in set {
            let name = match &attr.r#type {
                ty if Oid::JOINT_ISO_ITU_T_DS_ATTRIBUTE_TYPE_COMMON_NAME == *ty => "CN",
                ty if Oid::JOINT_ISO_ITU_T_DS_ATTRIBUTE_TYPE_COUNTRY_NAME == *ty => "C",
                ty if Oid::JOINT_ISO_ITU_T_DS_ATTRIBUTE_TYPE_LOCALITY_NAME == *ty => "L",
                ty if Oid::JOINT_ISO_ITU_T_DS_ATTRIBUTE_TYPE_STATE_OR_PROVINCE_NAME == *ty => "ST",
                ty if Oid::JOINT_ISO_ITU_T_DS_ATTRIBUTE_TYPE_ORGANISATION_NAME == *ty => "O",
                ty if Oid::JOINT_ISO_ITU_T_DS_ATTRIBUTE_TYPE_ORGANISATIONAL_UNIT_NAME == *ty => {
                    "OU"
                }
                oid => unimplemented!("{:?}", oid),
            };
            attrs.push(format!(
                "{}={}",
                name,
                std::str::from_utf8(&attr.value.as_bytes()[2..])?
            ));
        }
    }
    Ok(attrs.join(" "))
}
