use crate::devices::Device;
use crate::flutter::attach::VmService;
use anyhow::Result;

mod doctor;
mod new;

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

pub fn attach(url: &str) -> Result<()> {
    futures::executor::block_on(async move {
        let vm = VmService::attach(url).await?;
        let (major, minor) = vm.get_version().await?;
        println!("version {}.{}", major, minor);
        Ok(())
    })
}
