use crate::devices::Device;
use anyhow::Result;

mod attach;
mod doctor;
mod new;

pub use attach::attach;
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
