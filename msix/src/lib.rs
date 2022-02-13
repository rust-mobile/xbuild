use crate::block_map::BlockMapBuilder;
use crate::content_types::ContentTypesBuilder;
use crate::p7x::Digests;
use anyhow::Result;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use xcommon::{Signer, Zip, ZipFileOptions, ZipInfo};
use zip::ZipArchive;

mod block_map;
mod content_types;
pub mod manifest;
pub mod p7x;
mod pkcs7;

pub use crate::manifest::AppxManifest;

const DEBUG_KEY_PEM: &str = include_str!("../assets/debug.key.pem");
const DEBUG_CERT_PEM: &str = include_str!("../assets/debug.cert.pem");

pub struct Msix {
    path: PathBuf,
    zip: Zip,
}

impl Msix {
    pub fn new(path: PathBuf) -> Result<Self> {
        Ok(Self {
            zip: Zip::new(&path)?,
            path,
        })
    }

    pub fn add_manifest(&mut self, manifest: &AppxManifest) -> Result<()> {
        self.zip.create_file(
            "AppxManifest.xml".as_ref(),
            ZipFileOptions::Compressed,
            &to_xml(manifest),
        )
    }

    pub fn add_file(&mut self, source: &Path, dest: &Path, opts: ZipFileOptions) -> Result<()> {
        self.zip.add_file(source, dest, opts)
    }

    pub fn add_directory(&mut self, source: &Path, dest: &Path) -> Result<()> {
        self.zip.add_directory(source, dest)
    }

    pub fn finish(self, signer: Option<Signer>) -> Result<()> {
        self.zip.finish()?;
        Self::sign(&self.path, signer)
    }

    pub fn sign(path: &Path, signer: Option<Signer>) -> Result<()> {
        let signer = signer
            .map(Ok)
            .unwrap_or_else(|| Signer::new(DEBUG_KEY_PEM, DEBUG_CERT_PEM))
            .unwrap();

        // add content types and block map
        let mut zip = ZipArchive::new(BufReader::new(File::open(path)?))?;
        let mut content_types = ContentTypesBuilder::default();
        let mut block_map = BlockMapBuilder::default();
        for i in 0..zip.len() {
            let mut file = zip.by_index(i)?;
            content_types.add(file.name().as_ref());
            block_map.add(file.name().to_string(), file.size(), &mut file)?;
        }
        let content_types = to_xml(&content_types.finish());
        let axct = Sha256::digest(&content_types);
        let block_map = to_xml(&block_map.finish());
        let axbm = Sha256::digest(&block_map);
        let mut zip = Zip::append(path)?;
        zip.create_file(
            "[Content_Types].xml".as_ref(),
            ZipFileOptions::Compressed,
            &content_types,
        )?;
        zip.create_file(
            "AppxBlockMap.xml".as_ref(),
            ZipFileOptions::Compressed,
            &block_map,
        )?;
        zip.finish()?;

        // compute zip hashes
        let mut r = BufReader::new(File::open(path)?);
        let info = ZipInfo::new(&mut r)?;
        r.seek(SeekFrom::Start(0))?;
        let mut hasher = Sha256::new();
        let mut pc = (&mut r).take(info.cd_start);
        std::io::copy(&mut pc, &mut hasher)?;
        let axpc = hasher.finalize_reset();
        hasher.reset();
        std::io::copy(&mut r, &mut hasher)?;
        let axcd = hasher.finalize();
        let digests = Digests {
            axpc: axpc.into(),
            axcd: axcd.into(),
            axct: axct.into(),
            axbm: axbm.into(),
            ..Default::default()
        };

        // sign zip
        let sig = p7x::p7x(&signer, &digests);
        let mut zip = Zip::append(path)?;
        zip.create_file(
            "AppxSignature.p7x".as_ref(),
            ZipFileOptions::Compressed,
            &sig,
        )?;
        zip.finish()?;
        Ok(())
    }
}

fn to_xml<T: Serialize>(xml: &T) -> Vec<u8> {
    let mut buf = vec![];
    buf.extend_from_slice(r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#.as_bytes());
    quick_xml::se::to_writer(&mut buf, xml).unwrap();
    buf
}
