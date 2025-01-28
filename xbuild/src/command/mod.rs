use crate::cargo::CrateType;
use crate::devices::Device;
use crate::{BuildEnv, CompileTarget, Platform};
use anyhow::Result;
use app_store_connect::UnifiedApiKey;
use std::path::Path;

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
            format!("{} {}", device.platform()?, device.arch()?),
            device.details()?,
        );
    }
    Ok(())
}

pub fn run(env: &BuildEnv, launch_args: &[String]) -> Result<()> {
    let out = env.executable();
    if let Some(device) = env.target().device() {
        device.run(env, &out, launch_args)?;
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

pub fn create_apple_api_key(
    issuer_id: &str,
    key_id: &str,
    private_key: &Path,
    api_key: &Path,
) -> Result<()> {
    UnifiedApiKey::from_ecdsa_pem_path(issuer_id, key_id, private_key)?.write_json_file(api_key)?;
    Ok(())
}
