use crate::manifest::AppxManifest;
use anyhow::Result;
use serde::Serialize;
use std::io::{Cursor, Read, Seek, Write};
use xcommon::{Signer, ZipFileOptions};
use zip::write::{FileOptions, ZipWriter};

mod block_map;
mod content_types;
pub mod manifest;
mod p7x;
mod pkcs7;

pub struct MsixBuilder<W: Write + Seek> {
    zip: ZipWriter<W>,
    block_map: block_map::BlockMapBuilder,
    content_types: content_types::ContentTypesBuilder,
}

impl<W: Write + Seek> MsixBuilder<W> {
    pub fn new(w: W) -> Self {
        Self {
            zip: ZipWriter::new(w),
            block_map: Default::default(),
            content_types: Default::default(),
        }
    }

    pub fn add_manifest(&mut self, manifest: &AppxManifest) -> Result<()> {
        self.add_xml_file("AppxManifest.xml", manifest)
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
        self.content_types.add(name.as_ref());
        let zopts = FileOptions::default().compression_method(opts.compression_method());
        self.zip.start_file_aligned(name, zopts, opts.alignment())?;
        Ok(())
    }

    pub fn sign(mut self, signer: &Signer) -> Result<()> {
        let content_types = self.content_types.finish();
        let block_map = self.block_map.finish();
        self.add_xml_file("[Content_Types].xml", &content_types)?;
        self.add_xml_file("AppxBlockMap.xml", &block_map)?;
        // TODO: compute hashes
        let hashes = [[0; 32]; 5];
        self.add_file(
            "AppxSignature.p7x",
            ZipFileOptions::Compressed,
            &mut Cursor::new(p7x::p7x(signer, &hashes)),
        )?;
        self.zip.finish()?;
        Ok(())
    }
}
