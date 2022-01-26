use crate::apk::bxml::Xml;
use crate::{Signer, ZipFileOptions};
use anyhow::Result;
use serde::Serialize;
use std::fs::File;
use std::io::{Seek, Write};
use std::path::Path;
use zip::read::ZipFile;
use zip::write::{FileOptions, ZipWriter};

pub mod bxml;
pub mod manifest;
pub mod mipmap;
pub mod sign;

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
        //self.add_xml_file("AndroidManifest.xml", manifest)
        Ok(())
    }

    pub fn add_file(&mut self, path: &Path, name: &str, opts: ZipFileOptions) -> Result<()> {
        let mut f = File::open(path)?;
        self.start_file(name, opts)?;
        std::io::copy(&mut f, &mut self.zip)?;
        Ok(())
    }

    pub fn add_file_from_archive(&mut self, f: ZipFile) -> Result<()> {
        self.zip.raw_copy_file(f)?;
        Ok(())
    }

    pub fn add_directory(&mut self, source: &Path, dest: Option<&Path>) -> Result<()> {
        let dest = if let Some(dest) = dest {
            dest
        } else {
            Path::new("")
        };
        add_recursive(self, source, dest)?;
        Ok(())
    }

    fn add_xml_file<T: Serialize>(&mut self, name: &str, xml: &T) -> Result<()> {
        self.start_file(name, ZipFileOptions::Compressed)?;
        self.zip
            .write_all(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#.as_bytes())?;
        quick_xml::se::to_writer(&mut self.zip, xml)?;
        Ok(())
    }

    fn start_file(&mut self, name: &str, opts: ZipFileOptions) -> Result<()> {
        let zopts = FileOptions::default().compression_method(opts.compression_method());
        self.zip.start_file_aligned(name, zopts, opts.alignment())?;
        Ok(())
    }

    pub fn sign(mut self, _signer: &Signer) -> Result<()> {
        self.zip.finish()?;
        // TODO: sign
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
