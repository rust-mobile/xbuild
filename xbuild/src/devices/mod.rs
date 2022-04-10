use crate::devices::adb::Adb;
use crate::devices::host::Host;
use crate::devices::imd::IMobileDevice;
use crate::{Arch, Platform};
use anyhow::Result;
use std::path::Path;
use std::process::{Child, Command};

mod adb;
mod host;
mod imd;

#[derive(Clone, Debug)]
enum Backend {
    Adb(Adb),
    Imd(IMobileDevice),
    Host(Host),
}

#[derive(Clone, Debug)]
pub struct Device {
    backend: Backend,
    id: String,
}

impl std::str::FromStr for Device {
    type Err = anyhow::Error;

    fn from_str(device: &str) -> Result<Self> {
        if device == "host" {
            return Ok(Self::host());
        }
        if let Some((backend, id)) = device.split_once(':') {
            let backend = match backend {
                "adb" => Backend::Adb(Adb::which()?),
                "imd" => Backend::Imd(IMobileDevice::which()?),
                _ => anyhow::bail!("unsupported backend {}", backend),
            };
            Ok(Self {
                backend,
                id: id.to_string(),
            })
        } else {
            anyhow::bail!("invalid device identifier {}", device);
        }
    }
}

impl std::fmt::Display for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self.backend {
            Backend::Adb(_) => write!(f, "adb:{}", &self.id),
            Backend::Host(_) => write!(f, "{}", &self.id),
            Backend::Imd(_) => write!(f, "imd:{}", &self.id),
        }
    }
}

impl Device {
    pub fn list() -> Result<Vec<Self>> {
        let mut devices = vec![Self::host()];
        if let Ok(adb) = Adb::which() {
            adb.devices(&mut devices)?;
        }
        if let Ok(imd) = IMobileDevice::which() {
            imd.devices(&mut devices).ok();
        }
        Ok(devices)
    }

    pub fn host() -> Self {
        Self {
            backend: Backend::Host(Host),
            id: "host".to_string(),
        }
    }

    pub fn is_host(&self) -> bool {
        matches!(&self.backend, Backend::Host(_))
    }

    pub fn name(&self) -> Result<String> {
        match &self.backend {
            Backend::Adb(adb) => adb.name(&self.id),
            Backend::Host(host) => host.name(),
            Backend::Imd(imd) => imd.name(&self.id),
        }
    }

    pub fn platform(&self) -> Result<Platform> {
        match &self.backend {
            Backend::Adb(adb) => adb.platform(&self.id),
            Backend::Host(host) => host.platform(),
            Backend::Imd(imd) => imd.platform(&self.id),
        }
    }

    pub fn arch(&self) -> Result<Arch> {
        match &self.backend {
            Backend::Adb(adb) => adb.arch(&self.id),
            Backend::Host(host) => host.arch(),
            Backend::Imd(imd) => imd.arch(&self.id),
        }
    }

    pub fn details(&self) -> Result<String> {
        match &self.backend {
            Backend::Adb(adb) => adb.details(&self.id),
            Backend::Host(host) => host.details(),
            Backend::Imd(imd) => imd.details(&self.id),
        }
    }

    pub fn run(&self, path: &Path, attach: bool) -> Result<Runner> {
        let runner = match &self.backend {
            Backend::Adb(adb) => adb.run(&self.id, path, attach, false),
            Backend::Host(host) => host.run(path, attach),
            Backend::Imd(imd) => imd.run(&self.id, path, attach),
        }?;
        Ok(Runner::new(self.clone(), runner))
    }

    pub fn lldb(&self, executable: &Path, lldb_server: Option<&Path>) -> Result<()> {
        match &self.backend {
            Backend::Adb(adb) => {
                if let Some(lldb_server) = lldb_server {
                    adb.lldb(&self.id, executable, lldb_server)
                } else {
                    anyhow::bail!("lldb-server required on android");
                }
            }
            Backend::Host(host) => host.lldb(executable),
            Backend::Imd(imd) => imd.lldb(&self.id, executable),
        }
    }

    pub fn attach(&self, url: &str, root_dir: &Path, target: &Path) -> Result<()> {
        let port = url
            .strip_prefix("http://127.0.0.1:")
            .unwrap()
            .split_once('/')
            .unwrap()
            .0
            .parse()?;
        let host_vmservice_port = match &self.backend {
            Backend::Adb(adb) => Some(adb.forward(&self.id, port)?),
            _ => None,
        };
        // TODO: finish porting flutter attach to rust
        //crate::command::attach(url, root_dir, target, host_vmservice_port).await?;
        let device = match &self.backend {
            Backend::Host(host) => host.platform()?.to_string(),
            _ => self.id.to_string(),
        };
        let mut attach = Command::new("flutter");
        attach
            .current_dir(root_dir)
            .arg("attach")
            .arg("--device-id")
            .arg(device)
            .arg("--debug-url")
            .arg(url)
            .arg("--target")
            .arg(target);
        if let Some(port) = host_vmservice_port {
            attach.arg("--host-vmservice-port").arg(port.to_string());
        }
        attach.status()?;
        Ok(())
    }
}

pub(crate) struct PartialRunner {
    url: Option<String>,
    logger: Box<dyn FnOnce() + Send>,
    child: Option<Child>,
}

#[must_use]
pub struct Runner {
    device: Device,
    url: Option<String>,
    logger: Box<dyn FnOnce() + Send>,
    child: Option<Child>,
}

impl Runner {
    fn new(device: Device, runner: PartialRunner) -> Self {
        Self {
            device,
            url: runner.url,
            logger: runner.logger,
            child: runner.child,
        }
    }

    pub fn wait(self) {
        (self.logger)();
    }

    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    pub fn attach(self, root_dir: &Path, target: &Path) -> Result<()> {
        if let Some(url) = self.url.as_ref() {
            std::thread::spawn(self.logger);
            self.device.attach(url, root_dir, target)?;
        } else {
            self.wait();
        }
        Ok(())
    }

    pub fn kill(self) -> Result<()> {
        if let Some(mut child) = self.child {
            child.kill()?;
        }
        Ok(())
    }
}
