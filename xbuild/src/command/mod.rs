use crate::cargo::CrateType;
use crate::devices::Device;
use crate::{BuildEnv, CompileTarget, Platform};
use anyhow::Result;

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
        if env.target().platform() == Platform::Ios {
            let (major, minor) = device.ios_product_version()?;
            let disk_image = env.developer_disk_image(major, minor);
            device.ios_mount_disk_image(&disk_image)?;
        }
        device.run(&out)?;
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
            Platform::Ios => env.output().join("main"),
            Platform::Linux => env.output().join(env.name()),
            Platform::Macos => env.executable(),
            Platform::Windows => todo!(),
        };
        let lldb_server = match target.platform() {
            Platform::Android => Some(env.lldb_server(target)?),
            _ => None,
        };
        device.lldb(&executable, lldb_server.as_deref())?;
    } else {
        anyhow::bail!("no device specified");
    }
    Ok(())
}
