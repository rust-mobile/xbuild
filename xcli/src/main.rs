use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use xcommon::Signer;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
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
    target: String,
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

#[derive(Clone, Copy, Debug)]
enum Format {
    App,
    Apk,
    Appimage,
    Dmg,
    Ipa,
    Msix,
}

impl Format {
    fn from_path(path: &Path) -> Result<Self> {
        let ext = path
            .extension()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default()
            .to_lowercase();
        Ok(match ext.as_str() {
            "apk" => Format::Apk,
            "appimage" => Format::Appimage,
            "msix" => Format::Msix,
            ext => anyhow::bail!("unrecognized extension {}", ext),
        })
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    match args.command {
        Command::Build { build, sign } => {
            let path = cmd_build_and_sign(build, sign)?;
            println!("built {}", path.display());
        }
        Command::Sign { sign, file } => {
            cmd_sign(sign, &file)?;
        }
        Command::Verify { file } => {
            cmd_verify(&file)?;
        }
        Command::Run { build, sign, run } => {
            let path = cmd_build_and_sign(build, sign)?;
            cmd_run(&path, run)?;
        }
    }
    Ok(())
}

fn cmd_build_and_sign(build: BuildOptions, sign: SignOptions) -> Result<PathBuf> {
    let format = match build.target.as_str() {
        "aarch64-apple-ios" => Format::App,
        "aarch64-linux-android" => Format::Apk,
        "x86_64-apple-darwin" => Format::App,
        "x86_64-pc-windows-msvc" => Format::Msix,
        "x86_64-unknown-linux-gnu" => Format::Appimage,
        target => anyhow::bail!("unsupported target {}", target),
    };
    let signer = sign.signer()?;
    match format {
        Format::Apk => {}
        f => unimplemented!("{:?}", f),
    }
    //Ok(())
    panic!()
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
