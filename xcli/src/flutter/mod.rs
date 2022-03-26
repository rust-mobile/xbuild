use self::assets::AssetBundle;
use crate::{task, Arch, BuildEnv, CompileTarget, Opt, Platform};
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;

mod android;
pub mod artifacts;
pub mod assets;
pub mod attach;
mod ios;

pub struct Flutter {
    git: PathBuf,
    sdk: PathBuf,
    verbose: bool,
}

impl Flutter {
    pub fn new(sdk: PathBuf, verbose: bool) -> Result<Self> {
        let git = which::which("git")?;
        Ok(Self { git, sdk, verbose })
    }

    pub fn version(&self) -> Result<String> {
        let output = Command::new(&self.git)
            .current_dir(self.sdk.join("flutter"))
            .arg("tag")
            .arg("--points-at")
            .arg("HEAD")
            .output()?;
        if !output.status.success() {
            anyhow::bail!("failed to get flutter version");
        }
        let version = std::str::from_utf8(&output.stdout)?;
        Ok(version.to_string())
    }

    pub fn upgrade(&self) -> Result<()> {
        let flutter = self.sdk.join("flutter");
        if !flutter.exists() {
            std::fs::create_dir_all(&self.sdk)?;
            let mut cmd = Command::new(&self.git);
            cmd.current_dir(&self.sdk)
                .arg("clone")
                .arg("https://github.com/flutter/flutter")
                .arg("--depth")
                .arg("1")
                .arg("--branch")
                .arg("stable");
            task::run(cmd, self.verbose)?;
        } else {
            let mut cmd = Command::new(&self.git);
            cmd.current_dir(&flutter).arg("pull");
            task::run(cmd, self.verbose)?;
        }
        Ok(())
    }

    fn artifact_version(&self, artifact: &str) -> Result<String> {
        let path = self
            .sdk
            .join("flutter")
            .join("bin")
            .join("internal")
            .join(format!("{}.version", artifact));
        if !path.exists() {
            anyhow::bail!("failed to locate engine.version at {}", path.display());
        }
        Ok(std::fs::read_to_string(path)?.trim().into())
    }

    pub fn engine_version(&self) -> Result<String> {
        self.artifact_version("engine")
    }

    pub fn material_fonts_version(&self) -> Result<String> {
        Ok(self
            .artifact_version("material_fonts")?
            .split('/')
            .nth(3)
            .unwrap()
            .to_string())
    }

    pub fn engine_dir(&self, target: CompileTarget) -> Result<PathBuf> {
        let path = self
            .sdk
            .join("engine")
            .join(self.engine_version()?)
            .join(target.opt().to_string())
            .join(target.platform().to_string())
            .join(target.arch().to_string());
        Ok(path)
    }

    fn host_file(&self, path: &Path) -> Result<PathBuf> {
        let host = CompileTarget::new(Platform::host()?, Arch::host()?, Opt::Debug);
        let path = self.engine_dir(host)?.join(path);
        if !path.exists() {
            anyhow::bail!("failed to locate {}", path.display());
        }
        Ok(path)
    }

    pub fn material_fonts(&self) -> Result<PathBuf> {
        let dir = self.sdk.join("material_fonts");
        let version = self.material_fonts_version()?;
        Ok(dir.join(version))
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

    pub fn dart(&self) -> Result<Command> {
        let path = Path::new("dart-sdk").join("bin").join(exe!("dart"));
        Ok(Command::new(self.host_file(&path)?))
    }

    pub fn pub_get(&self, root_dir: &Path) -> Result<()> {
        let flutter_root = self.sdk.join("flutter");
        let version = self.version()?;
        std::fs::write(flutter_root.join("version"), version)?;
        let pkg_dir = flutter_root.join("bin").join("cache").join("pkg");
        std::fs::create_dir_all(&pkg_dir)?;
        let src_dir = self.host_file(Path::new("sky_engine"))?;
        let dest_dir = pkg_dir.join("sky_engine");
        if dest_dir.exists() {
            symlink::remove_symlink_dir(&dest_dir)?;
        }
        symlink::symlink_dir(&src_dir, &dest_dir)?;
        let mut cmd = self.dart()?;
        cmd.current_dir(root_dir)
            .env("FLUTTER_ROOT", flutter_root)
            .arg("pub")
            .arg("get")
            .arg("--no-precompile");
        task::run(cmd, self.verbose)?;
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

    pub fn kernel_blob_bin(
        &self,
        root_dir: &Path,
        target_file: &Path,
        output: &Path,
        depfile: &Path,
        opt: Opt,
    ) -> Result<()> {
        let mut cmd = self.dart()?;
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
        cmd.arg(target_file);
        task::run(cmd, self.verbose)?;
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
        cmd.arg("--deterministic").arg(kernel_blob_bin);
        task::run(cmd, self.verbose)?;
        Ok(())
    }

    pub fn build_classes_dex(&self, env: &BuildEnv, r8: &Path, deps: Vec<PathBuf>) -> Result<()> {
        android::build_classes_dex(env, r8, deps)
    }

    pub fn build_ios_main(&self, env: &BuildEnv, target: CompileTarget) -> Result<()> {
        ios::build_ios_main(env, self, target)
    }
}
