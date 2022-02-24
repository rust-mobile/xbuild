use crate::{Arch, CompileTarget, Opt, Platform};
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Flutter {
    path: PathBuf,
    engine: PathBuf,
}

impl Flutter {
    pub fn new(engine: PathBuf) -> Result<Self> {
        let path = dunce::canonicalize(which::which("flutter")?)?
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        Ok(Self { path, engine })
    }

    pub fn engine_version_path(&self) -> Result<PathBuf> {
        let path = self
            .path
            .join("bin")
            .join("internal")
            .join("engine.version");
        if !path.exists() {
            anyhow::bail!("failed to locate engine.version at {}", path.display());
        }
        Ok(path)
    }

    pub fn engine_version(&self) -> Result<String> {
        Ok(std::fs::read_to_string(self.engine_version_path()?)?
            .trim()
            .into())
    }

    pub fn engine_dir(&self, target: CompileTarget) -> Result<PathBuf> {
        let path = self.engine
            .join(self.engine_version()?)
            .join(target.opt().to_string())
            .join(target.platform().to_string())
            .join(target.arch().to_string());
        Ok(path)
    }

    pub fn pub_get(&self, root_dir: &Path) -> Result<()> {
        let status = Command::new("flutter")
            .current_dir(root_dir)
            .arg("pub")
            .arg("get")
            .arg("--suppress-analytics")
            .status()?;
        if !status.success() {
            anyhow::bail!("flutter pub get exited with status {:?}", status);
        }
        Ok(())
    }

    pub fn build_flutter_assets(
        &self,
        root_dir: &Path,
        flutter_assets: &Path,
        depfile: &Path,
    ) -> Result<()> {
        // in release mode only the assets are copied. this means that the result
        // should be platform independent.
        let host = CompileTarget::new(Platform::host()?, Arch::host()?, Opt::Release);
        let target_platform = match (host.platform(), host.arch()) {
            (Platform::Linux, Arch::Arm64) => "linux-arm64",
            (Platform::Linux, Arch::X64) => "linux-x64",
            (Platform::Macos, _) => "darwin",
            (Platform::Windows, Arch::X64) => "windows-x64",
            _ => anyhow::bail!(
                "unsupported platform arch combination {} {}",
                host.platform(),
                host.arch(),
            ),
        };
        let status = Command::new("flutter")
            .current_dir(root_dir)
            .arg("assemble")
            .arg("--no-version-check")
            .arg("--suppress-analytics")
            .arg("--depfile")
            .arg(depfile)
            .arg("--output")
            .arg(flutter_assets)
            .arg(format!("-dTargetPlatform={}", target_platform))
            .arg("-dBuildMode=release")
            .arg("copy_flutter_bundle")
            .status()?;
        if !status.success() {
            anyhow::bail!("flutter assemble exited with {:?}", status.code());
        }
        Ok(())
    }

    fn host_file(&self, path: &Path) -> Result<PathBuf> {
        let host = CompileTarget::new(Platform::host()?, Arch::host()?, Opt::Debug);
        let path = self.engine_dir(host)?.join(path);
        if !path.exists() {
            anyhow::bail!("failed to locate {}", path.display());
        }
        Ok(path)
    }

    pub fn icudtl_dat(&self) -> Result<PathBuf> {
        self.host_file(Path::new("icudtl.dat"))
    }

    pub fn isolate_snapshot_data(&self) -> Result<PathBuf> {
        self.host_file(Path::new("isolate_snapshot.bin"))
    }

    pub fn vm_snapshot_data(&self) -> Result<PathBuf> {
        self.host_file(Path::new("vm_isolate_snapshot.bin"))
    }

    pub fn kernel_blob_bin(
        &self,
        root_dir: &Path,
        target_file: &Path,
        output: &Path,
        depfile: &Path,
        opt: Opt,
    ) -> Result<()> {
        let mut cmd = Command::new(self.path.join("bin").join("dart"));
        cmd.current_dir(root_dir)
            .arg(self.host_file(Path::new("frontend_server.dart.snapshot"))?)
            .arg("--sdk-root")
            .arg(
                self.path
                    .join("bin")
                    .join("cache")
                    .join("artifacts")
                    .join("engine")
                    .join("common")
                    .join("flutter_patched_sdk"),
            )
            .arg("--target=flutter")
            .arg("--no-print-incremental-dependencies")
            .arg("--packages")
            .arg(".packages")
            .arg("--output-dill")
            .arg(output)
            .arg("--depfile")
            .arg(depfile);
        match opt {
            Opt::Release => {
                cmd.arg("-Ddart.vm.profile=false")
                    .arg("-Ddart.vm.product=true")
                    .arg("--aot")
                    .arg("--tfa");
            }
            Opt::Debug => {
                cmd.arg("-Ddart.vm.profile=false")
                    .arg("-Ddart.vm.product=true")
                    .arg("--track-widget-creation");
            }
        }
        let status = cmd.arg(target_file).status()?;
        if !status.success() {
            anyhow::bail!("failed to build kernel_blob.bin");
        }
        Ok(())
    }

    pub fn aot_snapshot(
        &self,
        root_dir: &Path,
        kernel_blob_bin: &Path,
        snapshot: &Path,
        target: CompileTarget,
    ) -> Result<()> {
        let gen_snapshot = match target.platform() {
            Platform::Linux => self.engine_dir(target)?.join("gen_snapshot"),
            Platform::Android => self
                .engine_dir(target)?
                .join("linux-x64")
                .join("gen_snapshot"),
            _ => unimplemented!(),
        };
        let mut cmd = Command::new(gen_snapshot);
        cmd.current_dir(root_dir);
        if target.platform() == Platform::Ios || target.platform() == Platform::Macos {
            cmd.arg("--snapshot_kind=app-aot-assembly")
                .arg(format!("--assembly={}", snapshot.display()));
        } else {
            cmd.arg("--snapshot_kind=app-aot-elf")
                .arg(format!("--elf={}", snapshot.display()));
        }
        let status = cmd.arg("--deterministic").arg(kernel_blob_bin).status()?;
        if !status.success() {
            anyhow::bail!("gen_snapshot failed with {:?}", status);
        }
        Ok(())
    }
}
