use crate::devices::{Backend, Device};
use crate::{Arch, Platform};
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug)]
pub(crate) struct IMobileDevice {
    idevice_id: PathBuf,
    ideviceinfo: PathBuf,
    ideviceimagemounter: PathBuf,
    ideviceinstaller: PathBuf,
    idevicedebug: PathBuf,
}

impl IMobileDevice {
    pub fn which() -> Result<Self> {
        Ok(Self {
            idevice_id: which::which(exe!("idevice_id"))?,
            ideviceinfo: which::which(exe!("ideviceinfo"))?,
            ideviceimagemounter: which::which(exe!("ideviceimagemounter"))?,
            ideviceinstaller: which::which(exe!("ideviceinstaller"))?,
            idevicedebug: which::which(exe!("idevicedebug"))?,
        })
    }

    fn getkey(&self, device: &str, key: &str) -> Result<String> {
        let output = Command::new(&self.ideviceinfo)
            .arg("--udid")
            .arg(device)
            .arg("--key")
            .arg(key)
            .output()?;
        anyhow::ensure!(output.status.success(), "failed to run ideviceinfo");
        Ok(std::str::from_utf8(&output.stdout)?.trim().to_string())
    }

    fn install(&self, device: &str, path: &Path) -> Result<()> {
        let status = Command::new(&self.ideviceinstaller)
            .arg("--udid")
            .arg(device)
            .arg("--install")
            .arg(path)
            .status()?;
        anyhow::ensure!(status.success(), "failed to run ideviceinstaller");
        Ok(())
    }

    fn start(&self, device: &str, bundle_identifier: &str) -> Result<()> {
        let status = Command::new(&self.idevicedebug)
            .arg("--udid")
            .arg(device)
            .arg("run")
            .arg(bundle_identifier)
            .status()?;
        anyhow::ensure!(status.success(), "failed to run idevicedebug");
        Ok(())
    }

    fn disk_image_mounted(&self, device: &str) -> Result<bool> {
        let output = Command::new(&self.ideviceimagemounter)
            .arg("--udid")
            .arg(device)
            .arg("-l")
            .output()?;
        anyhow::ensure!(output.status.success(), "failed to run ideviceimagemounter");
        let num_images: u32 = std::str::from_utf8(&output.stdout)?
            .split_once('[')
            .ok_or_else(|| anyhow::anyhow!("unexpected output"))?
            .1
            .split_once(']')
            .ok_or_else(|| anyhow::anyhow!("unexpected output"))?
            .0
            .parse()?;
        Ok(num_images > 0)
    }

    pub fn mount_disk_image(&self, device: &str, disk_image: &Path) -> Result<()> {
        if self.disk_image_mounted(device)? {
            return Ok(());
        }
        let status = Command::new(&self.ideviceimagemounter)
            .arg("--udid")
            .arg(device)
            .arg(disk_image)
            .status()?;
        anyhow::ensure!(status.success(), "failed to run ideviceimagemounter");
        Ok(())
    }

    pub fn run(&self, device: &str, path: &Path) -> Result<()> {
        let bundle_identifier = appbundle::app_bundle_identifier(path)?;
        self.install(device, path)?;
        self.start(device, &bundle_identifier)?;
        // TODO: log, attach
        Ok(())
    }

    pub fn devices(&self, devices: &mut Vec<Device>) -> Result<()> {
        let output = Command::new(&self.idevice_id)
            .arg("-l")
            .arg("-d")
            .output()?;
        anyhow::ensure!(output.status.success(), "failed to run idevice_id");
        let lines = std::str::from_utf8(&output.stdout)?.lines();
        for uuid in lines {
            devices.push(Device {
                backend: Backend::Imd(self.clone()),
                id: uuid.trim().to_string(),
            });
        }
        Ok(())
    }

    pub fn name(&self, device: &str) -> Result<String> {
        self.getkey(device, "DeviceName")
    }

    pub fn platform(&self, _device: &str) -> Result<Platform> {
        Ok(Platform::Ios)
    }

    pub fn arch(&self, device: &str) -> Result<Arch> {
        match self.getkey(device, "CPUArchitecture")?.as_str() {
            "arm64" => Ok(Arch::Arm64),
            arch => anyhow::bail!("unsupported arch {}", arch),
        }
    }

    pub fn product_version(&self, device: &str) -> Result<(u32, u32)> {
        let version = self.getkey(device, "ProductVersion")?;
        let (major, version) = version
            .split_once('.')
            .ok_or_else(|| anyhow::anyhow!("invalid product version"))?;
        let (minor, _) = version
            .split_once('.')
            .ok_or_else(|| anyhow::anyhow!("invalid product version"))?;
        Ok((major.parse()?, minor.parse()?))
    }

    pub fn details(&self, device: &str) -> Result<String> {
        let name = self.getkey(device, "ProductName")?;
        let version = self.getkey(device, "ProductVersion")?;
        Ok(format!("{} {}", name, version))
    }

    pub fn lldb(&self, _device: &str, _executable: &Path) -> Result<()> {
        anyhow::bail!("unimplemented");
    }
}
