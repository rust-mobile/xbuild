use crate::manifest::AppxManifest;
use anyhow::Result;
use serde::Serialize;
use std::io::Write;
use std::path::Path;
use xcommon::{Signer, Zip, ZipFileOptions};

mod block_map;
mod content_types;
pub mod manifest;
pub mod p7x;
mod pkcs7;

pub struct Msix {
    zip: Zip,
    block_map: block_map::BlockMapBuilder,
}

impl Msix {
    pub fn new(path: &Path) -> Result<Self> {
        Ok(Self {
            zip: Zip::new(path)?,
            block_map: Default::default(),
        })
    }

    pub fn add_manifest(&mut self, manifest: &AppxManifest) -> Result<()> {
        self.add_xml_file("AppxManifest.xml".as_ref(), manifest)
    }

    pub fn add_file(&mut self, source: &Path, dest: &Path, opts: ZipFileOptions) -> Result<()> {
        self.zip.add_file(source, dest, opts)
    }

    pub fn add_directory(&mut self, source: &Path, dest: &Path) -> Result<()> {
        self.zip.add_directory(source, dest)
    }

    fn add_xml_file<T: Serialize>(&mut self, dest: &Path, xml: &T) -> Result<()> {
        self.zip.start_file(dest, ZipFileOptions::Compressed)?;
        self.zip
            .write_all(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#.as_bytes())?;
        quick_xml::se::to_writer(&mut self.zip, xml)?;
        Ok(())
    }

    pub fn sign(mut self, signer: Option<Signer>) -> Result<()> {
        //let content_types = self.content_types.finish();
        let block_map = self.block_map.finish();

        //self.add_xml_file("[Content_Types].xml".as_ref(), &content_types)?;
        self.add_xml_file("AppxBlockMap.xml".as_ref(), &block_map)?;
        // TODO: compute hashes
        let hashes = [[0; 32]; 5];
        let sig = p7x::p7x(signer, &hashes);
        self.zip.create_file(
            "AppxSignature.p7x".as_ref(),
            ZipFileOptions::Compressed,
            &sig,
        )?;
        self.zip.finish()?;
        Ok(())
    }
}
