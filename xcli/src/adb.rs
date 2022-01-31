use crate::Device;
use anyhow::Result;
use regex::{Regex, RegexSet};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};

pub struct Adb(PathBuf);

impl Adb {
    pub fn which() -> Result<Self> {
        Ok(Self(which::which("adb")?))
    }

    pub fn serials(&self) -> Result<Vec<String>> {
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

    pub fn getprop(&self, device: &str, prop: &str) -> Result<String> {
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

    pub fn install(&self, device: &str, path: &Path) -> Result<()> {
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
    pub fn start(&self, device: &str, package: &str, activity: &str) -> Result<()> {
        let status = Command::new(&self.0)
            .arg("-s")
            .arg(device)
            .arg("shell")
            .arg("am")
            .arg("start")
            .arg("-a")
            .arg("android.intent.action.RUN")
            .arg("-n")
            .arg(format!("{}/{}", package, activity))
            .status()?;
        if !status.success() {
            anyhow::bail!("adb shell am start exited with code {:?}", status.code());
        }
        Ok(())
    }

    pub fn stop(&self, device: &str, id: &str) -> Result<()> {
        let status = Command::new(&self.0)
            .arg("-s")
            .arg(device)
            .arg("shell")
            .arg("am")
            .arg("force-stop")
            .arg(id)
            .status()?;
        if !status.success() {
            anyhow::bail!("adb shell am force-stop exited with code {:?}", status.code());
        }
        Ok(())
    }

    fn logcat_last_timestamp(&self) -> Result<String> {
        let output = Command::new(&self.0)
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
        let mut lines = std::str::from_utf8(&output.stdout)?.lines();
        lines.next();
        Ok(lines.next().unwrap().split_whitespace().nth(1).unwrap().to_string())
    }

    pub fn logcat<F: LogcatFilter>(&self, filter: F) -> Result<Logcat<F>> {
        let last_timestamp = self.logcat_last_timestamp()?;
        let child = Command::new(&self.0)
            .arg("shell")
            .arg("-x")
            .arg("logcat")
            .arg("-T")
            .arg(last_timestamp)
            .spawn()?;
        Ok(Logcat::new(child, filter))
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

    pub fn flutter_attach(&self, device: &str, package: &str, activity: &str) -> Result<()> {
        self.stop(device, package)?;
        self.start(device, package, activity)?;
        std::thread::sleep(std::time::Duration::from_secs(1));
        let output = Command::new(&self.0)
            .arg("shell")
            .arg("-x")
            .arg("logcat")
            .arg("-d")
            .arg("flutter:I")
            .arg("*:S")
            .stdout(Stdio::piped())
            .output()?;
        if !output.status.success() {
            anyhow::bail!("adb logcat exited with code {:?}", output.status.code());
        }
        let url = std::str::from_utf8(&output.stdout)?
            .lines()
            .rev()
            .filter_map(|line| {
                if let Some(url) = line.split_whitespace().last() {
                    if url.starts_with("http://127.0.0.1:") {
                        return Some(url.to_string());
                    }
                }
                None
            })
            .next();
        let url = url.ok_or_else(|| anyhow::anyhow!("failed to get debug url"))?;
        let status = Command::new("flutter")
            .arg("attach")
            .arg("--debug-url")
            .arg(url)
            .status()?;
        if !status.success() {
            anyhow::anyhow!("flutter attach failed with exit code {:?}", status.code());
        }
        Ok(())
    }

    pub fn devices(&self) -> Result<Vec<Device>> {
        let mut devices = vec![];
        for id in self.serials()? {
            let name = self.getprop(&id, "ro.product.device")?;
            let target = match self.getprop(&id, "ro.product.cpu.abi")?.as_str() {
                "arm64-v8a" => "aarch64-linux-android",
                "armeabi-v7a" => "armv7-linux-androideabi",
                "x86_64" => "x86_64-linux-android",
                "x86" => "i686-linux-android",
                abi => anyhow::bail!("unrecognized abi {}", abi),
            };
            let release = self.getprop(&id, "ro.build.version.release")?;
            let sdk = self.getprop(&id, "ro.build.version.sdk")?;
            let platform = format!("Android {} (API {})", release, sdk);
            devices.push(Device {
                name,
                id,
                target: target.to_string(),
                platform,
            });
        }
        Ok(devices)
    }
}

pub trait LogcatFilter {
    fn accept_line(&mut self, line: &str) -> bool;
}

pub struct Logcat<F: LogcatFilter> {
    child: Child,
    reader: BufReader<ChildStdout>,
    line: String,
    filter: F,
}

impl<F: LogcatFilter> Logcat<F> {
    fn new(mut child: Child, filter: F) -> Self {
        let stdout = child.stdout.take().expect("child missing stdout");
        let reader = BufReader::new(stdout);
        Self {
            child,
            reader,
            line: String::with_capacity(1024),
            filter,
        }
    }
}

impl<F: LogcatFilter> Iterator for Logcat<F> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            self.line.clear();
            self.reader.read_line(&mut self.line).ok()?;
            loop {
                if self.filter.accept_line(&self.line) {
                    return Some(self.line.clone());
                }
            }
        }
    }
}

impl<F: LogcatFilter> Drop for Logcat<F> {
    fn drop(&mut self) {
        self.child.kill().ok();
    }
}

#[derive(Default)]
pub struct NoFilter;

impl LogcatFilter for NoFilter {
    fn accept_line(&mut self, _line: &str) -> bool {
        true
    }
}

/*pub struct FlutterLogcatFilter {
    fatal_crash: bool,
    accepted_last_line: bool,
    log_format: Regex,
    fatal_log: Regex,
    tombstone_line: Regex,
    tombstone_terminator: Regex,
    allowed_tags: RegexSet,
}

impl Default for FlutterLogcatFilter {
    fn default() -> Self {
        Self {
            fatal_crash: false,
            accepted_last_line: false,
            log_format: Regex::new(r"^[VDIWEF]\/.*?\(\s*(\d+)\):\s").unwrap(),
            fatal_log: Regex::new(r"^F\/libc\s*\(\s*\d+\):\sFatal signal (\d+)").unwrap(),
            tombstone_line: Regex::new(r"^[IF]\/DEBUG\s*\(\s*\d+\):\s(.+)$").unwrap(),
            tombstone_terminator: Regex::new(r"^Tombstone written to:\s").unwrap(),
            allowed_tags: RegexSet::new(&[
                r"^[VDIWEF]\/flutter[^:]*:\s+",
                r"^[IE]\/DartVM[^:]*:\s+",
                r"^[WEF]\/AndroidRuntime:\s+",
                r"^[WEF]\/AndroidRuntime\([0-9]+\):\s+",
                r"^[WEF]\/ActivityManager:\s+.*(\bflutter\b|\bdomokit\b|\bsky\b)",
                r"^[WEF]\/System\.err:\s+",
                r"^[F]\/[\S^:]+:\s+",
            ])
            .unwrap(),
        }
    }
}

impl LogcatFilter for FlutterLogcatFilter {
    fn accept_line(&mut self, line: &str) -> bool {
        let log_match = self.log_format.matches(&self.line);
        self.accepted_last_line = if self.log_format.is_match(&self.line) {
            if self.fatal_crash {
                if self.tombstone_line.is_match(&self.line) {
                    if self.tombstone_terminator.is_match(&self.line) {
                        self.fatal_crash = false;
                    }
                    true
                } else {
                    // only accept lines that are part of the crash report
                    false
                }
            } else if self.app_pid == log_format.nth(1) {
                if self.fatal_log.is_match(&self.line) {
                    self.fatal_crash = true;
                }
                true
            } else {
                self.allowed_tags.is_match(&self.line)
            }
        } else if self.line == "-------- beginning of system"
            || self.line == "-------- beginning of main"
        {
            false
        } else {
            // If it doesn't match the log pattern at all, then pass it through if
            // we passed the last matching line through. It might be a multiline
            // message.
            self.accepted_last_line
        };
        if self.accepted_last_line {
            return Some(self.line.clone());
        }
        self.accepted_last_line
    }
}*/
