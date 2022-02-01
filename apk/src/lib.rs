use anyhow::Result;
use std::fs::File;
use std::io::{Seek, Write};
use std::path::Path;
use xcommon::ZipFileOptions;
use zip::write::{FileOptions, ZipWriter};

pub mod manifest;
pub mod manifestc;
pub mod mipmap;
pub mod res;
pub mod sign;

pub use manifestc::Xml;

pub enum Abi {
    ArmV7a,
    ArmV8a,
    X86,
    X86_64,
}

impl std::fmt::Display for Abi {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let abi = match self {
            Self::ArmV7a => "armeabi-v7a",
            Self::ArmV8a => "arm64-v8a",
            Self::X86 => "x86",
            Self::X86_64 => "x86_64",
        };
        write!(f, "{}", abi)
    }
}

pub struct ApkBuilder<W: Write + Seek> {
    zip: ZipWriter<W>,
}

impl<W: Write + Seek> ApkBuilder<W> {
    pub fn new(w: W) -> Self {
        Self {
            zip: ZipWriter::new(w),
        }
    }

    pub fn add_manifest(&mut self, manifest: &Xml) -> Result<()> {
        let bxml = manifest.compile()?;
        self.start_file("AndroidManifest.xml", ZipFileOptions::Compressed)?;
        self.zip.write_all(&bxml)?;
        Ok(())
    }

    pub fn add_dex(&mut self, dex: &[u8]) -> Result<()> {
        self.start_file("classes.dex", ZipFileOptions::Compressed)?;
        self.zip.write_all(&dex)?;
        Ok(())
    }

    pub fn add_icon(&mut self, icon: &Path) -> Result<()> {
        Ok(())
    }

    pub fn add_lib(&mut self, abi: Abi, path: &Path) -> Result<()> {
        let name = path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("invalid path"))?
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("invalid path"))?;
        let mut f = File::open(path)?;
        self.start_file(&format!("lib/{}/{}", abi, name), ZipFileOptions::Compressed)?;
        std::io::copy(&mut f, &mut self.zip)?;
        Ok(())
    }

    pub fn add_assets(&mut self, dir: &Path) -> Result<()> {
        self.add_directory(dir, Some(Path::new("assets")))?;
        Ok(())
    }

    fn add_file(&mut self, path: &Path, name: &str, opts: ZipFileOptions) -> Result<()> {
        let mut f = File::open(path)?;
        self.start_file(name, opts)?;
        std::io::copy(&mut f, &mut self.zip)?;
        Ok(())
    }

    fn add_directory(&mut self, source: &Path, dest: Option<&Path>) -> Result<()> {
        let dest = if let Some(dest) = dest {
            dest
        } else {
            Path::new("")
        };
        add_recursive(self, source, dest)?;
        Ok(())
    }

    fn start_file(&mut self, name: &str, opts: ZipFileOptions) -> Result<()> {
        let zopts = FileOptions::default().compression_method(opts.compression_method());
        self.zip.start_file_aligned(name, zopts, opts.alignment())?;
        Ok(())
    }

    pub fn build(mut self) -> Result<()> {
        self.zip.finish()?;
        Ok(())
    }
}

fn add_recursive<W: Write + Seek>(
    builder: &mut ApkBuilder<W>,
    source: &Path,
    dest: &Path,
) -> Result<()> {
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let source = source.join(&file_name);
        let dest = dest.join(&file_name);
        let file_type = entry.file_type()?;
        let dest_str = dest.to_str().unwrap();
        if file_type.is_dir() {
            add_recursive(builder, &source, &dest)?;
        } else if file_type.is_file() {
            builder.add_file(&source, dest_str, ZipFileOptions::Compressed)?;
        }
    }
    Ok(())
}
