use crate::res::Chunk;
use anyhow::Result;
use std::fs::File;
use std::io::{BufWriter, Cursor, Write};
use std::path::{Path, PathBuf};
use xcommon::ZipFileOptions;
use zip::write::{FileOptions, ZipWriter};

mod compiler;
pub mod manifest;
mod res;
mod sign;

pub use crate::manifest::AndroidManifest;
pub use xcommon::{Certificate, Signer};

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

pub struct Resources {
    manifest: Chunk,
    resources: Option<Chunk>,
}

impl Resources {
    pub fn new(manifest: &AndroidManifest, icon: Option<&Path>) -> Result<Self> {
        crate::compiler::compile(manifest, icon)
    }
}

pub struct Apk {
    path: PathBuf,
    zip: ZipWriter<BufWriter<File>>,
}

impl Apk {
    pub fn new(path: PathBuf) -> Result<Self> {
        let zip = ZipWriter::new(BufWriter::new(File::create(&path)?));
        Ok(Self { path, zip })
    }

    pub fn add_res(&mut self, res: &Resources) -> Result<()> {
        self.start_file("AndroidManifest.xml", ZipFileOptions::Compressed)?;
        let mut buf = vec![];
        let mut cursor = Cursor::new(&mut buf);
        res.manifest.write(&mut cursor)?;
        self.zip.write_all(&buf)?;
        if let Some(res) = &res.resources {
            self.start_file("resources.arsc", ZipFileOptions::Aligned(4))?;
            buf.clear();
            let mut cursor = Cursor::new(&mut buf);
            res.write(&mut cursor)?;
            self.zip.write_all(&buf)?;
        }
        Ok(())
    }

    pub fn add_dex(&mut self, dex: &[u8]) -> Result<()> {
        self.start_file("classes.dex", ZipFileOptions::Compressed)?;
        self.zip.write_all(&dex)?;
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

    pub fn finish(mut self, signer: Option<Signer>) -> Result<()> {
        self.zip.finish()?;
        crate::sign::sign(&self.path, signer)?;
        Ok(())
    }

    pub fn sign(path: &Path, signer: Option<Signer>) -> Result<()> {
        crate::sign::sign(&path, signer)
    }

    pub fn verify(path: &Path) -> Result<Vec<Certificate>> {
        crate::sign::verify(path)
    }
}

fn add_recursive(builder: &mut Apk, source: &Path, dest: &Path) -> Result<()> {
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
