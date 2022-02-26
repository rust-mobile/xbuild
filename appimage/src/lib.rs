use anyhow::Result;
use std::fs::{File, Permissions};
use std::io::{BufReader, BufWriter, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use xcommon::Signer;

static RUNTIME: &[u8] = include_bytes!("../assets/runtime-x86_64");

pub struct AppImage {
    appdir: PathBuf,
    name: String,
}

impl AppImage {
    pub fn new(build_dir: &Path, name: String) -> Result<Self> {
        let appdir = build_dir.join(format!("{}.AppDir", name));
        std::fs::remove_dir_all(&appdir).ok();
        std::fs::create_dir_all(&appdir)?;
        Ok(Self { appdir, name })
    }

    pub fn appdir(&self) -> &Path {
        &self.appdir
    }

    pub fn add_apprun(&self) -> Result<()> {
        let apprun = self.appdir.join("AppRun");
        let mut f = File::create(&apprun)?;
        writeln!(f, "#!/bin/sh")?;
        writeln!(f, r#"cd "$(dirname "$0")""#)?;
        writeln!(f, "LD_LIBRARY_PATH=./lib exec ./{}", self.name)?;
        #[cfg(unix)]
        std::fs::set_permissions(apprun, Permissions::from_mode(0o755))?;
        Ok(())
    }

    pub fn add_desktop(&self) -> Result<()> {
        let mut f = File::create(self.appdir.join(format!("{}.desktop", &self.name)))?;
        writeln!(f, "[Desktop Entry]")?;
        writeln!(f, "Version=1.0")?;
        writeln!(f, "Type=Application")?;
        writeln!(f, "Terminal=false")?;
        writeln!(f, "Name={}", self.name)?;
        writeln!(f, "Exec={} %u", self.name)?;
        writeln!(f, "Icon={}", self.name)?;
        writeln!(f, "Categories=Utility;")?;
        Ok(())
    }

    pub fn add_icon(&self, path: &Path) -> Result<()> {
        let ext = path
            .extension()
            .map(|ext| ext.to_str())
            .unwrap_or_default()
            .ok_or_else(|| anyhow::anyhow!("unsupported extension"))?;
        let name = format!("{}.{}", self.name, ext);
        self.add_file(path, Path::new(&name))?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(name, self.appdir.join(".DirIcon"))?;
        Ok(())
    }

    pub fn add_file(&self, path: &Path, name: &Path) -> Result<()> {
        let dest = self.appdir.join(name);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(path, dest)?;
        Ok(())
    }

    pub fn add_directory(&self, source: &Path, dest: &Path) -> Result<()> {
        let dest = self.appdir.join(dest);
        std::fs::create_dir_all(&dest)?;
        xcommon::copy_dir_all(source, &dest)?;
        Ok(())
    }

    pub fn build(self, out: &Path, _signer: Option<Signer>) -> Result<()> {
        let squashfs = self
            .appdir
            .parent()
            .unwrap()
            .join(format!("{}.squashfs", self.name));
        let status = Command::new("mksquashfs")
            .arg(&self.appdir)
            .arg(&squashfs)
            .arg("-root-owned")
            .arg("-noappend")
            .status()?;
        if !status.success() {
            anyhow::bail!("mksquashfs failed with exit code {:?}", status);
        }
        let mut squashfs = BufReader::new(File::open(squashfs)?);
        let mut f = File::create(out)?;
        #[cfg(unix)]
        f.set_permissions(Permissions::from_mode(0o755))?;
        let mut out = BufWriter::new(&mut f);
        out.write_all(RUNTIME)?;
        std::io::copy(&mut squashfs, &mut out)?;
        // TODO: sign
        Ok(())
    }
}
