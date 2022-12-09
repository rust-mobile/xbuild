use crate::cargo::CrateType;
use crate::devices::Device;
use crate::{BuildEnv, CompileTarget, Platform};
use anyhow::Result;
use pem::Pem;
use rand::rngs::OsRng;
use rsa::pkcs8::{EncodePrivateKey, LineEnding};
use rsa::RsaPrivateKey;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use x509_certificate::{InMemorySigningKeyPair, Sign, X509CertificateBuilder};

mod build;
mod doctor;
mod new;

pub use build::build;
pub use doctor::doctor;
pub use new::new;

pub fn devices() -> Result<()> {
    for device in Device::list()? {
        println!(
            "{:50}{:20}{:20}{}",
            device.to_string(),
            device.name()?,
            format_args!("{} {}", device.platform()?, device.arch()?),
            device.details()?,
        );
    }
    Ok(())
}

pub fn run(env: &BuildEnv) -> Result<()> {
    let out = env.executable();
    if let Some(device) = env.target().device() {
        device.run(env, &out)?;
    } else {
        anyhow::bail!("no device specified");
    }
    Ok(())
}

pub fn lldb(env: &BuildEnv) -> Result<()> {
    if let Some(device) = env.target().device() {
        let target = CompileTarget::new(device.platform()?, device.arch()?, env.target().opt());
        let cargo_dir = env
            .build_dir()
            .join(target.opt().to_string())
            .join(target.platform().to_string())
            .join(target.arch().to_string())
            .join("cargo");
        let executable = match target.platform() {
            Platform::Android => env.cargo_artefact(&cargo_dir, target, CrateType::Cdylib)?,
            Platform::Ios => env.output(),
            Platform::Linux => env.output().join(env.name()),
            Platform::Macos => env.executable(),
            Platform::Windows => todo!(),
        };
        let lldb_server = match target.platform() {
            Platform::Android => Some(env.lldb_server(target)?),
            _ => None,
        };
        device.lldb(env, &executable, lldb_server.as_deref())?;
    } else {
        anyhow::bail!("no device specified");
    }
    Ok(())
}

pub fn generate_key(pem: &Path) -> Result<()> {
    RsaPrivateKey::new(&mut OsRng, 2048)?.write_pkcs8_pem_file(pem, LineEnding::CRLF)?;
    Ok(())
}

pub fn generate_csr(pem: &Path, csr: &Path) -> Result<()> {
    let pem = std::fs::read_to_string(pem)?;
    let pem = pem::parse_many(pem)?;
    let key = if let Some(key) = pem.iter().find(|pem| pem.tag == "PRIVATE KEY") {
        InMemorySigningKeyPair::from_pkcs8_der(&key.contents)?
    } else {
        anyhow::bail!("no private key found");
    };
    let mut builder = X509CertificateBuilder::new(key.key_algorithm().unwrap());
    builder
        .subject()
        .append_common_name_utf8_string("Apple Code Signing CSR")
        .expect("only valid chars");
    let pem = builder
        .create_certificate_signing_request(&key)?
        .encode_pem()?;
    std::fs::write(csr, pem)?;
    Ok(())
}

pub fn add_certificate(pem: &Path, cer: &Path) -> Result<()> {
    let mut f = OpenOptions::new().write(true).append(true).open(pem)?;
    let cer = std::fs::read(cer)?;
    let pem = pem::encode(&Pem {
        tag: "CERTIFICATE".into(),
        contents: cer,
    });
    f.write_all(pem.as_bytes())?;
    Ok(())
}
