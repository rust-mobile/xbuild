use crate::config::Config;
use crate::devices::{Backend, Device};
use anyhow::Result;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};

#[derive(Clone)]
pub struct Adb(PathBuf);

impl Adb {
    pub fn which() -> Result<Self> {
        Ok(Self(which::which("adb")?))
    }

    fn serials(&self) -> Result<Vec<String>> {
        let output = Command::new(&self.0).arg("devices").output()?;
        if !output.status.success() {
            anyhow::bail!("adb devices exited with code {:?}", output.status.code());
        }
        let mut lines = std::str::from_utf8(&output.stdout)?.lines();
        lines.next();
        let mut devices = vec![];
        for line in lines {
            if let Some(id) = line.split_whitespace().next() {
                devices.push(id.to_string());
            }
        }
        Ok(devices)
    }

    fn getprop(&self, device: &str, prop: &str) -> Result<String> {
        let output = Command::new(&self.0)
            .arg("-s")
            .arg(device)
            .arg("shell")
            .arg("getprop")
            .arg(prop)
            .output()?;
        if !output.status.success() {
            anyhow::bail!("adb getprop exited with code {:?}", output.status.code());
        }
        Ok(std::str::from_utf8(&output.stdout)?.trim().to_string())
    }

    fn install(&self, device: &str, path: &Path) -> Result<()> {
        let status = Command::new(&self.0)
            .arg("-s")
            .arg(device)
            .arg("install")
            .arg(path)
            .status()?;
        if !status.success() {
            anyhow::bail!("adb install exited with code {:?}", status.code());
        }
        Ok(())
    }

    /// To run a native activity use "android.app.NativeActivity" as the activity name.
    fn start(&self, device: &str, package: &str, activity: &str) -> Result<()> {
        let status = Command::new(&self.0)
            .arg("-s")
            .arg(device)
            .arg("shell")
            .arg("am")
            .arg("start")
            .arg("-a")
            .arg("android.intent.action.RUN")
            .arg("-n")
            .arg(format!("{}/.{}", package, activity))
            .status()?;
        if !status.success() {
            anyhow::bail!("adb shell am start exited with code {:?}", status.code());
        }
        Ok(())
    }

    fn stop(&self, device: &str, id: &str) -> Result<()> {
        let status = Command::new(&self.0)
            .arg("-s")
            .arg(device)
            .arg("shell")
            .arg("am")
            .arg("force-stop")
            .arg(id)
            .status()?;
        if !status.success() {
            anyhow::bail!(
                "adb shell am force-stop exited with code {:?}",
                status.code()
            );
        }
        Ok(())
    }

    fn logcat_last_timestamp(&self, device: &str) -> Result<String> {
        let output = Command::new(&self.0)
            .arg("-s")
            .arg(device)
            .arg("shell")
            .arg("-x")
            .arg("logcat")
            .arg("-v")
            .arg("time")
            .arg("-t")
            .arg("1")
            .output()?;
        if !output.status.success() {
            anyhow::bail!("adb logcat exited with code {:?}", output.status.code());
        }
        let line = std::str::from_utf8(&output.stdout)?
            .lines()
            .skip(1)
            .next()
            .unwrap();
        Ok(line[..18].to_string())
    }

    fn pidof(&self, device: &str, id: &str) -> Result<u32> {
        std::thread::sleep(std::time::Duration::from_secs(1));
        let output = Command::new(&self.0)
            .arg("-s")
            .arg(device)
            .arg("shell")
            .arg("-x")
            .arg("pidof")
            .arg(id)
            .output()?;
        if !output.status.success() {
            anyhow::bail!("failed to get pid");
        }
        Ok(String::from_utf8(output.stdout)?.trim().parse()?)
    }

    fn logcat(&self, device: &str, pid: u32, last_timestamp: &str) -> Result<Logcat> {
        let child = Command::new(&self.0)
            .arg("-s")
            .arg(device)
            .arg("shell")
            .arg("-x")
            .arg("logcat")
            .arg("-T")
            .arg(format!("'{}'", last_timestamp))
            .arg(format!("--pid={}", pid))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()?;
        Ok(Logcat::new(child))
    }

    pub fn forward(&self, device: &str, port: u16) -> Result<u16> {
        let output = Command::new(&self.0)
            .arg("-s")
            .arg(device)
            .arg("forward")
            .arg("tcp:0")
            .arg(format!("tcp:{}", port))
            .output()?;
        if !output.status.success() {
            anyhow::bail!("adb forward exited with code {:?}", output.status.code());
        }
        Ok(std::str::from_utf8(&output.stdout)?.trim().parse()?)
    }

    pub fn run(
        &self,
        device: &str,
        path: &Path,
        config: &Config,
        flutter_attach: bool,
    ) -> Result<()> {
        let package = &config.apk.manifest.package;
        let activity = config
            .apk
            .manifest
            .application
            .activity
            .name
            .rsplit_once('.')
            .ok_or_else(|| anyhow::anyhow!("invalid activity name"))?
            .1;
        self.stop(device, package)?;
        self.install(device, path)?;
        let last_timestamp = self.logcat_last_timestamp(device)?;
        self.start(device, package, activity)?;
        let pid = self.pidof(device, package)?;
        let mut logcat = self.logcat(device, pid, &last_timestamp)?;
        if flutter_attach {
            let url = loop {
                if let Some(line) = logcat.next() {
                    if let Some((_, url)) = line.rsplit_once(' ') {
                        if url.starts_with("http") {
                            break url.trim().to_string();
                        }
                    }
                    println!("{}", line.trim());
                }
            };
            let port = url
                .strip_prefix("http://127.0.0.1:")
                .unwrap()
                .split_once('/')
                .unwrap()
                .0
                .parse()?;
            let port = self.forward(device, port)?;
            println!("attaching to {} {}", url, port);
            std::thread::spawn(move || {
                for line in logcat {
                    println!("{}", line);
                }
            });
            Command::new("flutter")
                .arg("attach")
                .arg("--debug-url")
                .arg(url)
                .arg("--host-vmservice-port")
                .arg(port.to_string())
                .status()?;
        } else {
            for line in logcat {
                println!("{}", line);
            }
        }
        Ok(())
    }

    pub fn devices(&self, devices: &mut Vec<Device>) -> Result<()> {
        for id in self.serials()? {
            devices.push(Device {
                backend: Backend::Adb(self.clone()),
                id,
            });
        }
        Ok(())
    }

    pub fn name(&self, device: &str) -> Result<String> {
        self.getprop(device, "ro.product.device")
    }

    pub fn target(&self, device: &str) -> Result<&'static str> {
        let target = match self.getprop(device, "ro.product.cpu.abi")?.as_str() {
            "arm64-v8a" => "aarch64-linux-android",
            "armeabi-v7a" => "armv7-linux-androideabi",
            "x86_64" => "x86_64-linux-android",
            "x86" => "i686-linux-android",
            abi => anyhow::bail!("unrecognized abi {}", abi),
        };
        Ok(target)
    }

    pub fn platform(&self, device: &str) -> Result<String> {
        let release = self.getprop(device, "ro.build.version.release")?;
        let sdk = self.getprop(device, "ro.build.version.sdk")?;
        Ok(format!("Android {} (API {})", release, sdk))
    }
}

pub struct Logcat {
    child: Child,
    reader: BufReader<ChildStdout>,
    line: String,
}

impl Logcat {
    fn new(mut child: Child) -> Self {
        let stdout = child.stdout.take().expect("child missing stdout");
        let reader = BufReader::new(stdout);
        Self {
            child,
            reader,
            line: String::with_capacity(1024),
        }
    }
}

impl Iterator for Logcat {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            self.line.clear();
            match self.reader.read_line(&mut self.line) {
                Ok(0) => return None,
                Ok(_) => {
                    return Some(self.line[20..].to_string());
                }
                Err(_) => {}
            }
        }
    }
}

impl Drop for Logcat {
    fn drop(&mut self) {
        self.child.kill().ok();
    }
}
