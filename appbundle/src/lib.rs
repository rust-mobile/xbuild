use anyhow::Result;
use icns::{IconFamily, Image};
use std::fs::File;
use std::io::{BufWriter, Cursor};
use std::path::{Path, PathBuf};
use std::process::Command;
use xcommon::{Scaler, ScalerOpts, Signer};

mod info;

pub use info::InfoPlist;

const ICON_SIZES: [u32; 6] = [16, 32, 64, 128, 256, 512];

pub struct AppBundle {
    appdir: PathBuf,
    info: InfoPlist,
}

impl AppBundle {
    pub fn new(build_dir: &Path, info: InfoPlist) -> Result<Self> {
        let appdir = build_dir.join(format!("{}.app", &info.name));
        std::fs::remove_dir_all(&appdir).ok();
        std::fs::create_dir_all(&appdir)?;
        Ok(Self { appdir, info })
    }

    pub fn appdir(&self) -> &Path {
        &self.appdir
    }

    fn content_dir(&self) -> PathBuf {
        if self.info.requires_ios == Some(true) {
            self.appdir.to_path_buf()
        } else {
            self.appdir.join("Contents")
        }
    }

    fn resource_dir(&self) -> PathBuf {
        self.content_dir().join("Resources")
    }

    fn framework_dir(&self) -> PathBuf {
        self.content_dir().join("Frameworks")
    }

    fn executable_dir(&self) -> PathBuf {
        let contents = self.content_dir();
        if self.info.requires_ios == Some(true) {
            contents
        } else {
            contents.join("MacOS")
        }
    }

    pub fn add_icon(&mut self, path: &Path) -> Result<()> {
        let mut icns = IconFamily::new();
        let scaler = Scaler::open(path)?;
        let mut buf = vec![];
        for size in ICON_SIZES {
            buf.clear();
            let mut cursor = Cursor::new(&mut buf);
            scaler.write(&mut cursor, ScalerOpts::new(size))?;
            let image = Image::read_png(&*buf)?;
            icns.add_icon(&image)?;
        }
        let path = self.resource_dir().join("AppIcon.icns");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        icns.write(BufWriter::new(File::create(path)?))?;
        self.info.icon_file = Some("AppIcon".to_string());
        Ok(())
    }

    pub fn add_file(&self, path: &Path, dest: &Path) -> Result<()> {
        let dest = self.resource_dir().join(dest);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(path, dest)?;
        Ok(())
    }

    pub fn add_directory(&self, source: &Path, dest: &Path) -> Result<()> {
        let resource_dir = self.resource_dir().join(dest);
        std::fs::create_dir_all(&resource_dir)?;
        xcommon::copy_dir_all(source, &resource_dir)?;
        Ok(())
    }

    pub fn add_executable(&mut self, path: &Path) -> Result<()> {
        let file_name = path.file_name().unwrap().to_str().unwrap();
        let exe_dir = self.executable_dir();
        std::fs::create_dir_all(&exe_dir)?;
        std::fs::copy(path, exe_dir.join(file_name))?;
        if self.info.executable.is_none() {
            self.info.executable = Some(file_name.to_string());
        }
        Ok(())
    }

    pub fn add_framework(&self, path: &Path) -> Result<()> {
        let framework_dir = self.framework_dir().join(path.file_name().unwrap());
        std::fs::create_dir_all(&framework_dir)?;
        xcommon::copy_dir_all(path, &framework_dir)?;
        Ok(())
    }

    pub fn add_lib(&self, path: &Path) -> Result<()> {
        let file_name = path.file_name().unwrap();
        let framework_dir = self.framework_dir();
        std::fs::create_dir_all(&framework_dir)?;
        std::fs::copy(path, framework_dir.join(file_name))?;
        Ok(())
    }

    pub fn finish(self, _signer: Option<Signer>) -> Result<PathBuf> {
        let path = self.content_dir().join("Info.plist");
        plist::to_file_xml(path, &self.info)?;
        Ok(self.appdir)
    }
}

pub fn make_dmg(build_dir: &Path, appbundle: &Path, dmg: &Path) -> Result<()> {
    let name = dmg.file_stem().unwrap().to_str().unwrap();
    let uncompressed = build_dir.join(format!("{}.uncompressed.dmg", name));
    make_uncompressed_dmg(appbundle, &uncompressed, name)?;
    make_compressed_dmg(&uncompressed, dmg)?;
    Ok(())
}

fn make_uncompressed_dmg(appbundle: &Path, uncompressed_dmg: &Path, volname: &str) -> Result<()> {
    let status = Command::new("hdiutil")
        .arg("create")
        .arg(uncompressed_dmg)
        .arg("-ov")
        .arg("-volname")
        .arg(volname)
        .arg("-fs")
        .arg("HFS+")
        .arg("-srcfolder")
        .arg(appbundle)
        .status()?;
    if !status.success() {
        anyhow::bail!("failed to build uncompressed dmg");
    }
    Ok(())
}

fn make_compressed_dmg(uncompressed_dmg: &Path, dmg: &Path) -> Result<()> {
    let status = Command::new("hdiutil")
        .arg("convert")
        .arg(uncompressed_dmg)
        .arg("-format")
        .arg("UDZO")
        .arg("-o")
        .arg(dmg)
        .status()?;
    if !status.success() {
        anyhow::bail!("failed to build compressed dmg");
    }
    Ok(())
}
