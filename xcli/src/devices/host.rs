use crate::config::Config;
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

    pub fn target(&self) -> Result<&'static str> {
        Ok(if cfg!(target_os = "linux") {
            "x86_64-unknown-linux-gnu"
        } else if cfg!(target_os = "macos") {
            "x86_64-apple-darwin"
        } else if cfg!(target_os = "windows") {
            "x86_64-pc-windows-msvc"
        } else {
            anyhow::bail!("unsupported host");
        })
    }

    pub fn platform(&self) -> Result<String> {
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

    pub fn run(&self, path: &Path, _config: &Config, attach: bool) -> Result<()> {
        let mut child = Command::new(path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()?;
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        if attach {
            let line = lines.next().transpose()?;
            let url = line
                .as_ref()
                .map(|line| line.rsplit_once(' '))
                .flatten()
                .map(|(_, url)| url.to_string())
                .ok_or_else(|| anyhow::anyhow!("failed to get debug url"))?;
            std::thread::spawn(move || {
                Command::new("flutter")
                    .arg("attach")
                    .arg("--debug-url")
                    .arg(url)
                    .status()
            });
        }
        for line in lines {
            let line = line?;
            println!("{}", line);
        }
        Ok(())
    }
}
