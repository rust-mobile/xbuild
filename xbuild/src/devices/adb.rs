use crate::config::AndroidDebugConfig;
use crate::devices::{Backend, Device};
use crate::{Arch, Platform};
use anyhow::Result;
use apk::Apk;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::Duration;

#[derive(Clone, Debug)]
pub(crate) struct Adb(PathBuf);

impl Adb {
    pub fn which() -> Result<Self> {
        Ok(Self(which::which(exe!("adb"))?))
    }

    fn adb(&self, device: &str) -> Command {
        let mut cmd = Command::new(&self.0);
        cmd.arg("-s").arg(device);
        cmd
    }

    fn push(&self, device: &str, path: &Path) -> Result<()> {
        let status = self
            .adb(device)
            .arg("push")
            .arg("--sync")
            .arg(path)
            .arg("/data/local/tmp")
            .status()?;
        anyhow::ensure!(status.success(), "adb push failed");
        Ok(())
    }

    fn shell(&self, device: &str, run_as: Option<&str>) -> Command {
        let mut cmd = self.adb(device);
        cmd.arg("shell");
        if let Some(package) = run_as {
            cmd.arg("run-as").arg(package);
        }
        cmd
    }

    pub fn devices(&self, devices: &mut Vec<Device>) -> Result<()> {
        let output = Command::new(&self.0).arg("devices").output()?;
        anyhow::ensure!(
            output.status.success(),
            "adb devices exited with code {:?}: {}",
            output.status.code(),
            std::str::from_utf8(&output.stderr)?.trim()
        );
        let mut lines = std::str::from_utf8(&output.stdout)?.lines();
        lines.next();
        for line in lines {
            if let Some(id) = line.split_whitespace().next() {
                devices.push(Device {
                    backend: Backend::Adb(self.clone()),
                    id: id.to_string(),
                });
            }
        }
        Ok(())
    }

    fn getprop(&self, device: &str, prop: &str) -> Result<String> {
        let output = self.shell(device, None).arg("getprop").arg(prop).output()?;
        anyhow::ensure!(
            output.status.success(),
            "adb getprop exited with code {:?}: {}",
            output.status.code(),
            std::str::from_utf8(&output.stderr)?.trim()
        );
        Ok(std::str::from_utf8(&output.stdout)?.trim().to_string())
    }

    fn install(&self, device: &str, path: &Path) -> Result<()> {
        let file_name = path.file_name().unwrap().to_str().unwrap();
        self.push(device, path)?;
        let status = self
            .shell(device, None)
            .arg("pm")
            .arg("install")
            .arg(format!("/data/local/tmp/{}", file_name))
            .status()?;
        anyhow::ensure!(
            status.success(),
            "adb pm install exited with code {:?}",
            status.code()
        );
        Ok(())
    }

    /// To run a native activity use "android.app.NativeActivity" as the activity name
    fn start(&self, device: &str, package: &str, activity: &str) -> Result<()> {
        let status = self
            .shell(device, None)
            .arg("am")
            .arg("start")
            .arg("-W")
            .arg("-a")
            .arg("android.intent.action.MAIN")
            .arg("-n")
            .arg(format!("{}/{}", package, activity))
            .status()?;
        anyhow::ensure!(
            status.success(),
            "adb shell am start exited with code {:?}",
            status.code()
        );
        Ok(())
    }

    fn stop(&self, device: &str, id: &str) -> Result<()> {
        let status = self
            .shell(device, None)
            .arg("am")
            .arg("force-stop")
            .arg(id)
            .status()?;
        anyhow::ensure!(
            status.success(),
            "adb shell am force-stop exited with code {:?}",
            status.code()
        );
        Ok(())
    }

    fn forward_reverse(&self, device: &str, debug_config: &AndroidDebugConfig) -> Result<()> {
        for (local, remote) in &debug_config.forward {
            let status = self
                .adb(device)
                .arg("forward")
                .arg(local)
                .arg(remote)
                .status()?;
            anyhow::ensure!(
                status.success(),
                "adb forward exited with code {:?}",
                status.code()
            );
        }
        for (remote, local) in &debug_config.reverse {
            let status = self
                .adb(device)
                .arg("reverse")
                .arg(remote)
                .arg(local)
                .status()?;
            anyhow::ensure!(
                status.success(),
                "adb reverse exited with code {:?}",
                status.code()
            );
        }
        Ok(())
    }

    fn set_debug_app(&self, device: &str, package: &str) -> Result<()> {
        let status = self
            .shell(device, None)
            .arg("am")
            .arg("set-debug-app")
            .arg("-w")
            .arg(package)
            .status()?;
        anyhow::ensure!(
            status.success(),
            "adb shell am set-debug-app exited with code {:?}",
            status.code()
        );
        Ok(())
    }

    fn clear_debug_app(&self, device: &str) -> Result<()> {
        let status = self
            .shell(device, None)
            .arg("am")
            .arg("clear-debug-app")
            .status()?;
        anyhow::ensure!(
            status.success(),
            "adb shell am clear-debug-app exited with code {:?}",
            status.code()
        );
        Ok(())
    }

    fn logcat_last_timestamp(&self, device: &str) -> Result<String> {
        let output = self
            .shell(device, None)
            .arg("logcat")
            .arg("-v")
            .arg("time")
            .arg("-t")
            .arg("1")
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "adb logcat exited with code {:?}: {}",
            output.status.code(),
            std::str::from_utf8(&output.stderr)?.trim()
        );
        let line = std::str::from_utf8(&output.stdout)?.lines().nth(1).unwrap();
        Ok(line[..18].to_string())
    }

    fn pidof(&self, device: &str, id: &str) -> Result<u32> {
        loop {
            let output = self.shell(device, None).arg("pidof").arg(id).output()?;
            anyhow::ensure!(
                output.status.success(),
                "failed to get pid: {}",
                std::str::from_utf8(&output.stderr)?.trim()
            );
            let pid = std::str::from_utf8(&output.stdout)?.trim();
            // may return multiple space separated pids if the old process hasn't exited yet.
            if pid.is_empty() || pid.contains(' ') {
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }
            println!("pid of {} is {}", id, pid);
            return Ok(pid.parse()?);
        }
    }

    fn logcat(&self, device: &str, pid: u32, last_timestamp: &str) -> Result<Logcat> {
        let child = self
            .shell(device, None)
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
        let output = self
            .adb(device)
            .arg("forward")
            .arg("tcp:0")
            .arg(format!("tcp:{}", port))
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "adb forward exited with code {:?}: {}",
            output.status.code(),
            std::str::from_utf8(&output.stderr)?.trim()
        );
        Ok(std::str::from_utf8(&output.stdout)?.trim().parse()?)
    }

    /*fn app_dir(&self, device: &str, package: &str) -> Result<PathBuf> {
        let output = self
            .shell(device, Some(package))
            .arg("sh")
            .arg("-c")
            .arg("pwd")
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "failed to get app dir: {}",
            std::str::from_utf8(&output.stderr)?.trim()
        );
        Ok(Path::new(std::str::from_utf8(&output.stdout)?.trim()).to_path_buf())
    }*/

    pub fn lldb(&self, device: &str, lldb_server: &Path, executable: &Path) -> Result<()> {
        /*let package = env.manifest().android().package.as_ref().unwrap();
        let app_dir = self.app_dir(device, package)?;
        self.shell(device, Some(package))
            .arg("chmod")
            .arg("a+x")
            .arg(&app_dir)
            .status()?;
        let dest = app_dir.join("lldb-server");*/
        self.push(device, lldb_server)?;
        /*self.shell(device, None)
            .arg("cat")
            .arg("/data/local/tmp/lldb-server")
            .arg("|")
            .arg("run-as")
            .arg(package)
            .arg("sh")
            .arg("-c")
            .arg(format!("'cat > {}'", dest.display()))
            .status()?;
        self.shell(device, Some(package))
            .arg("chmod")
            .arg("700")
            .arg(&dest)
            .status()?;*/
        let mut lldb_server = self
            .shell(device, None)
            .arg("cd")
            .arg("/data/local/tmp")
            .arg("&&")
            .arg("./lldb-server")
            .arg("platform")
            .arg("--listen")
            .arg("*:10086")
            .arg("--server")
            .stdin(Stdio::null())
            .spawn()?;
        std::thread::sleep(Duration::from_millis(100));
        self.forward(device, 10086)?;
        let status = Command::new("lldb")
            .arg("-O")
            .arg("platform select remote-android")
            //.arg("-O")
            //.arg(format!("platform settings -w {}", app_dir.display()))
            //.arg("platform settings -w /data/local/tmp")
            .arg("-O")
            .arg(format!("platform connect connect://{}:10086", device))
            .arg(executable)
            .status()?;
        anyhow::ensure!(status.success(), "lldb exited with nonzero exit code.");
        lldb_server.kill()?;
        Ok(())
    }

    pub fn run(
        &self,
        device: &str,
        path: &Path,
        debug_config: &AndroidDebugConfig,
        debug: bool,
    ) -> Result<()> {
        let entry_point = Apk::entry_point(path)?;
        let package = &entry_point.package;
        let activity = &entry_point.activity;
        self.stop(device, package)?;
        if debug {
            self.set_debug_app(device, package)?;
        } else {
            self.clear_debug_app(device)?;
        }
        self.install(device, path)?;
        self.forward_reverse(device, debug_config)?;
        let last_timestamp = self.logcat_last_timestamp(device)?;
        self.start(device, package, activity)?;
        let pid = self.pidof(device, package)?;
        let logcat = self.logcat(device, pid, &last_timestamp)?;
        for line in logcat {
            println!("{}", line);
        }
        Ok(())
    }

    pub fn name(&self, device: &str) -> Result<String> {
        self.getprop(device, "ro.product.device")
    }

    pub fn platform(&self, _device: &str) -> Result<Platform> {
        Ok(Platform::Android)
    }

    pub fn arch(&self, device: &str) -> Result<Arch> {
        let arch = match self.getprop(device, "ro.product.cpu.abi")?.as_str() {
            "arm64-v8a" => Arch::Arm64,
            //"armeabi-v7a" => Arch::Arm,
            "x86_64" => Arch::X64,
            //"x86" => Arch::X86,
            abi => anyhow::bail!("unrecognized abi {}", abi),
        };
        Ok(arch)
    }

    pub fn details(&self, device: &str) -> Result<String> {
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
                    let line = self.line.trim();
                    if let Some((date, line)) = line.split_once(' ') {
                        if let Some((time, line)) = line.split_once(' ') {
                            if date.len() == 5 && time.len() == 12 {
                                return Some(line.to_string());
                            }
                        }
                    }
                    return Some(self.line.clone());
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
