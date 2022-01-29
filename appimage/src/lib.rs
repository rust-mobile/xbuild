use anyhow::{Context, Result};
use std::fs::{File, Permissions};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use xcommon::Signer;

pub struct AppImageBuilder {
    appdir: PathBuf,
    out: PathBuf,
    name: String,
}

impl AppImageBuilder {
    pub fn new(build_dir: &Path, out: &Path, name: String) -> Result<Self> {
        let appdir = build_dir.join(format!("{}.AppDir", name));
        std::fs::remove_dir_all(&appdir).ok();
        std::fs::create_dir_all(&appdir)?;
        Ok(Self {
            appdir,
            out: out.to_path_buf(),
            name,
        })
    }

    pub fn add_apprun(&self) -> Result<()> {
        let apprun = self.appdir.join("AppRun");
        let mut f = File::create(&apprun)?;
        writeln!(f, "#!/bin/sh")?;
        writeln!(f, r#"cd "$(dirname "$0")""#)?;
        writeln!(f, "exec ./{}", self.name)?;
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
        let mut f = File::open(path).context("failed to open icon file")?;
        let name = format!("{}.{}", self.name, ext);
        self.add_file(&name, &mut f)?;
        std::os::unix::fs::symlink(name, self.appdir.join(".DirIcon"))?;
        Ok(())
    }

    pub fn add_file(&self, name: &str, input: &mut impl Read) -> Result<()> {
        let mut f = File::create(self.appdir.join(name))?;
        std::io::copy(input, &mut f)?;
        Ok(())
    }

    pub fn add_directory(&self, source: &Path, dest: Option<&Path>) -> Result<()> {
        let dest = if let Some(dest) = dest {
            self.appdir.join(dest)
        } else {
            self.appdir.clone()
        };
        std::fs::create_dir_all(&dest)?;
        copy_recursive(source, &dest)?;
        Ok(())
    }

    pub fn sign(self, _signer: Option<Signer>) -> Result<()> {
        let status = Command::new("appimagetool")
            .arg(self.appdir)
            .arg(self.out)
            .status()?;
        if !status.success() {
            anyhow::bail!("failed to build appimage");
        }
        // TODO: sign
        Ok(())
    }
}

pub fn copy_recursive(source: &Path, dest: &Path) -> Result<()> {
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let source = source.join(&file_name);
        let dest = dest.join(&file_name);
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            std::fs::create_dir_all(&dest)?;
            copy_recursive(&source, &dest)?;
        } else if file_type.is_file() {
            std::fs::copy(&source, &dest)?;
        } else if file_type.is_symlink() {
            let target = std::fs::read_link(&source)?;
            std::os::unix::fs::symlink(target, dest)?;
        }
    }
    Ok(())
}
