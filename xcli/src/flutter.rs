use crate::assets::AssetBundle;
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
        if let Ok(version) = std::env::var("FLUTTER_ENGINE_VERSION") {
            return Ok(version);
        }
        Ok(std::fs::read_to_string(self.engine_version_path()?)?
            .trim()
            .into())
    }

    pub fn engine_dir(&self, target: CompileTarget) -> Result<PathBuf> {
        let path = self
            .engine
            .join(self.engine_version()?)
            .join(target.opt().to_string())
            .join(target.platform().to_string())
            .join(target.arch().to_string());
        Ok(path)
    }

    pub fn dart(&self) -> Command {
        let path = self
            .path
            .join("bin")
            .join("cache")
            .join("dart-sdk")
            .join("bin")
            .join(exe!("dart"));
        Command::new(path)
    }

    pub fn flutter(&self) -> Command {
        let path = self.path.join("bin");
        if cfg!(windows) {
            Command::new(path.join("flutter.bat"))
        } else {
            Command::new(path.join("flutter"))
        }
    }

    pub fn pub_get(&self, root_dir: &Path) -> Result<()> {
        let status = self
            .dart()
            .current_dir(root_dir)
            .env("FLUTTER_ROOT", &self.path)
            .arg("__deprecated_pub")
            .arg("get")
            .arg("--no-precompile")
            .status()?;
        if !status.success() {
            anyhow::bail!("dart pub get exited with status {:?}", status);
        }
        Ok(())
    }

    pub fn build_flutter_assets(
        &self,
        root_dir: &Path,
        flutter_assets: &Path,
        _depfile: &Path,
    ) -> Result<()> {
        let bundle = AssetBundle::new(root_dir, &self.material_fonts()?)?;
        bundle.assemble(flutter_assets)?;
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

    pub fn material_fonts(&self) -> Result<PathBuf> {
        let path = self
            .path
            .join("bin")
            .join("cache")
            .join("artifacts")
            .join("material_fonts");
        if !path.exists() {
            anyhow::bail!("failed to locate {}", path.display());
        }
        Ok(path)
    }

    pub fn kernel_blob_bin(
        &self,
        root_dir: &Path,
        target_file: &Path,
        output: &Path,
        depfile: &Path,
        opt: Opt,
    ) -> Result<()> {
        let mut cmd = self.dart();
        cmd.current_dir(root_dir)
            .arg(self.host_file(Path::new("frontend_server.dart.snapshot"))?)
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
                cmd.arg("--sdk-root")
                    .arg(self.host_file(Path::new("flutter_patched_sdk_product"))?)
                    .arg("-Ddart.vm.profile=false")
                    .arg("-Ddart.vm.product=true")
                    .arg("--aot")
                    .arg("--tfa");
            }
            Opt::Debug => {
                cmd.arg("--sdk-root")
                    .arg(self.host_file(Path::new("flutter_patched_sdk"))?)
                    .arg("-Ddart.vm.profile=false")
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
        let gen_snapshot = self.engine_dir(target)?.join(exe!("gen_snapshot"));
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
