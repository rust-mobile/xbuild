use crate::apk::manifest::AndroidManifest;
use crate::{Signer, ZipFileOptions};
use anyhow::Result;
use serde::Serialize;
use std::io::{Read, Seek, Write};
use zip::write::{FileOptions, ZipWriter};

pub mod manifest;
pub mod mipmap;
pub mod sign;

pub struct ApkBuilder<W: Write + Seek> {
    zip: ZipWriter<W>,
}

impl<W: Write + Seek> ApkBuilder<W> {
    pub fn new(w: W) -> Self {
        Self { zip: ZipWriter::new(w) }
    }

    pub fn add_manifest(&mut self, manifest: &AndroidManifest) -> Result<()> {
        self.add_xml_file("AndroidManifest.xml", manifest)
    }

    pub fn add_file(
        &mut self,
        name: &str,
        opts: ZipFileOptions,
        input: &mut impl Read,
    ) -> Result<()> {
        self.start_file(name, opts)?;
        std::io::copy(input, &mut self.zip)?;
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
