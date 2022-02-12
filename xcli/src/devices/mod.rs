use crate::devices::adb::Adb;
use crate::devices::host::Host;
use crate::{Arch, BuildEnv, Platform};
use anyhow::Result;
use std::path::Path;

mod adb;
mod host;

#[derive(Clone, Debug)]
enum Backend {
    Adb(Adb),
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
            Backend::Host(_) => write!(f, "{}", &self.id),
            Backend::Adb(_) => write!(f, "adb:{}", &self.id),
        }
    }
}

impl Device {
    pub fn list() -> Result<Vec<Self>> {
        let mut devices = vec![Self::host()];
        if let Ok(adb) = Adb::which() {
            adb.devices(&mut devices)?;
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
        if let Backend::Host(_) = &self.backend {
            true
        } else {
            false
        }
    }

    pub fn name(&self) -> Result<String> {
        match &self.backend {
            Backend::Adb(adb) => adb.name(&self.id),
            Backend::Host(host) => host.name(),
        }
    }

    pub fn platform(&self) -> Result<Platform> {
        match &self.backend {
            Backend::Adb(adb) => adb.platform(&self.id),
            Backend::Host(host) => host.platform(),
        }
    }

    pub fn arch(&self) -> Result<Arch> {
        match &self.backend {
            Backend::Adb(adb) => adb.arch(&self.id),
            Backend::Host(host) => host.arch(),
        }
    }

    pub fn details(&self) -> Result<String> {
        match &self.backend {
            Backend::Adb(adb) => adb.details(&self.id),
            Backend::Host(host) => host.details(),
        }
    }

    pub fn run(&self, path: &Path, env: &BuildEnv, attach: bool) -> Result<()> {
        match &self.backend {
            Backend::Adb(adb) => adb.run(&self.id, path, env, attach),
            Backend::Host(host) => host.run(path, env, attach),
        }
    }
}
