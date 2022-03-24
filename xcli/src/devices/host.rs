use crate::devices::Run;
use crate::{Arch, Platform};
use anyhow::Result;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Clone, Debug)]
pub struct Host;

impl Host {
    pub fn name(&self) -> Result<String> {
        if cfg!(target_os = "linux") {
            let output = Command::new("uname").output()?;
            if !output.status.success() {
                anyhow::bail!("uname failed");
            }
            let name = std::str::from_utf8(&output.stdout)?.trim();
            Ok(name.to_string())
        } else {
            Ok("host".to_string())
        }
    }

    pub fn platform(&self) -> Result<Platform> {
        Platform::host()
    }

    pub fn arch(&self) -> Result<Arch> {
        Arch::host()
    }

    pub fn details(&self) -> Result<String> {
        if cfg!(target_os = "linux") {
            let os_release = std::fs::read_to_string("/etc/os-release")?;
            let mut distro = os_release
                .lines()
                .filter_map(|line| line.split_once('='))
                .filter(|(k, _)| *k == "NAME")
                .map(|(_, v)| v.trim_matches('"').to_string())
                .next()
                .unwrap_or_default();
            let output = Command::new("uname").arg("-r").output()?;
            if !output.status.success() {
                anyhow::bail!("uname failed");
            }
            distro.push_str(" ");
            distro.push_str(std::str::from_utf8(&output.stdout)?.trim());
            Ok(distro)
        } else {
            Ok("".to_string())
        }
    }

    pub fn run(&self, path: &Path, flutter_attach: bool) -> Result<Run> {
        let mut child = Command::new(path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()?;
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let url = if flutter_attach {
            let url = loop {
                if let Some(line) = lines.next() {
                    let line = line?;
                    let line = line.trim();
                    if let Some((_, url)) = line.rsplit_once(' ') {
                        if url.starts_with("http://127.0.0.1") {
                            break url.trim().to_string();
                        }
                    }
                    println!("{}", line);
                }
            };
            Some(url)
        } else {
            None
        };
        Ok(Run {
            url,
            logger: Box::new(move || {
                for line in lines {
                    if let Ok(line) = line {
                        println!("{}", line.trim());
                    }
                }
            }),
            child: Some(child),
        })
    }
}
