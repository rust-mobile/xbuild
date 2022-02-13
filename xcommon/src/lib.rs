use anyhow::Result;
use image::imageops::FilterType;
use image::io::Reader as ImageReader;
use image::{DynamicImage, GenericImageView, ImageOutputFormat};
use rsa::pkcs8::FromPrivateKey;
use rsa::{Hash, PaddingScheme, RsaPrivateKey, RsaPublicKey};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufWriter, Seek, Write};
use std::path::Path;
use zip::write::FileOptions;
use zip::{CompressionMethod, ZipWriter};

pub use rasn_pkix::Certificate;
pub use zip::read::ZipFile;

pub struct Scaler {
    img: DynamicImage,
}

impl Scaler {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let img = ImageReader::open(path)?.decode()?;
        let (width, height) = img.dimensions();
        if width != height {
            anyhow::bail!("expected width == height");
        }
        if width < 512 {
            anyhow::bail!("expected icon of at least 512x512 px");
        }
        Ok(Self { img })
    }

    pub fn optimize(&mut self) {
        let mut is_grayscale = true;
        let mut is_opaque = true;
        let (width, height) = self.img.dimensions();
        for x in 0..width {
            for y in 0..height {
                let pixel = self.img.get_pixel(x, y);
                if pixel[0] != pixel[1] || pixel[1] != pixel[2] {
                    is_grayscale = false;
                }
                if pixel[3] != 255 {
                    is_opaque = false;
                }
                if !is_grayscale && !is_opaque {
                    break;
                }
            }
        }
        match (is_grayscale, is_opaque) {
            (true, true) => self.img = DynamicImage::ImageLuma8(self.img.to_luma8()),
            (true, false) => self.img = DynamicImage::ImageLumaA8(self.img.to_luma_alpha8()),
            (false, true) => self.img = DynamicImage::ImageRgb8(self.img.to_rgb8()),
            (false, false) => {}
        }
    }

    pub fn write<W: Write + Seek>(&self, w: &mut W, size: u32) -> Result<()> {
        self.img
            .resize(size, size, FilterType::Nearest)
            .write_to(w, ImageOutputFormat::Png)?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct Signer {
    key: RsaPrivateKey,
    pubkey: RsaPublicKey,
    cert: Certificate,
}

impl Signer {
    /// Creates a new signer using a private key and a certificate.
    ///
    /// A new self signed certificate can be generated using openssl:
    /// ```sh
    /// openssl req -newkey rsa:2048 -new -nodes -x509 -days 3650 -keyout key.pem -out cert.pem
    /// ```
    pub fn new(private_key: &str, certificate: &str) -> Result<Self> {
        let key = RsaPrivateKey::from_pkcs8_pem(private_key)?;
        let pubkey = RsaPublicKey::from(&key);
        let pem = pem::parse(certificate)?;
        anyhow::ensure!(pem.tag == "CERTIFICATE");
        let cert = rasn::der::decode::<Certificate>(&pem.contents)
            .map_err(|err| anyhow::anyhow!("{}", err))?;
        Ok(Self { key, pubkey, cert })
    }

    pub fn sign(&self, bytes: &[u8]) -> Vec<u8> {
        let digest = Sha256::digest(bytes);
        let padding = PaddingScheme::new_pkcs1v15_sign(Some(Hash::SHA2_256));
        self.key.sign(padding, &digest).unwrap()
    }

    pub fn pubkey(&self) -> &RsaPublicKey {
        &self.pubkey
    }

    pub fn cert(&self) -> &Certificate {
        &self.cert
    }
}

impl std::fmt::Debug for Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Signer")
            .field("pubkey", &self.pubkey)
            .field("cert", &self.cert)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ZipFileOptions {
    Unaligned,
    Aligned(u16),
    Compressed,
}

impl ZipFileOptions {
    pub fn alignment(self) -> u16 {
        match self {
            Self::Aligned(align) => align,
            _ => 1,
        }
    }

    pub fn compression_method(&self) -> CompressionMethod {
        match self {
            Self::Compressed => CompressionMethod::Deflated,
            _ => CompressionMethod::Stored,
        }
    }
}

pub struct Zip(ZipWriter<BufWriter<File>>);

impl Zip {
    pub fn new(path: &Path) -> Result<Self> {
        Ok(Self(ZipWriter::new(BufWriter::new(File::create(path)?))))
    }

    pub fn add_file(&mut self, source: &Path, dest: &Path, opts: ZipFileOptions) -> Result<()> {
        let mut f = File::open(source)?;
        self.start_file(dest, opts)?;
        std::io::copy(&mut f, &mut self.0)?;
        Ok(())
    }

    pub fn add_directory(&mut self, source: &Path, dest: &Path) -> Result<()> {
        add_recursive(self, source, dest)?;
        Ok(())
    }

    pub fn add_zip_file(&mut self, f: ZipFile) -> Result<()> {
        self.0.raw_copy_file(f)?;
        Ok(())
    }

    pub fn create_file(&mut self, dest: &Path, opts: ZipFileOptions, contents: &[u8]) -> Result<()> {
        self.start_file(dest, opts)?;
        self.0.write_all(contents)?;
        Ok(())
    }

    pub fn start_file(&mut self, dest: &Path, opts: ZipFileOptions) -> Result<()> {
        let name = dest
            .iter()
            .map(|seg| seg.to_str().unwrap())
            .collect::<Vec<_>>()
            .join("/");
        let zopts = FileOptions::default().compression_method(opts.compression_method());
        self.0.start_file_aligned(name, zopts, opts.alignment())?;
        Ok(())
    }

    pub fn finish(mut self) -> Result<()> {
        self.0.finish()?;
        Ok(())
    }
}

fn add_recursive(zip: &mut Zip, source: &Path, dest: &Path) -> Result<()> {
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let source = source.join(&file_name);
        let dest = dest.join(&file_name);
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            add_recursive(zip, &source, &dest)?;
        } else if file_type.is_file() {
            zip.add_file(&source, &dest, ZipFileOptions::Compressed)?;
        }
    }
    Ok(())
}

impl Write for Zip {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.write(bytes)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

pub fn copy_dir_all(source: &Path, dest: &Path) -> Result<()> {
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let source = source.join(&file_name);
        let dest = dest.join(&file_name);
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            std::fs::create_dir_all(&dest)?;
            copy_dir_all(&source, &dest)?;
        } else if file_type.is_file() {
            std::fs::copy(&source, &dest)?;
        } else if file_type.is_symlink() {
            let target = std::fs::read_link(&source)?;
            std::os::unix::fs::symlink(target, dest)?;
        }
    }
    Ok(())
}

pub fn stamp_file(file: &Path, stamp: &Path) -> Result<bool> {
    let stamp_exists = stamp.exists();
    if !stamp_exists {
        if let Some(parent) = stamp.parent() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let stamp_time = File::create(stamp)?.metadata()?.modified()?;
    let file_time = File::open(file)?.metadata()?.modified()?;
    Ok(!stamp_exists || file_time > stamp_time)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = include_str!("../../assets/key.pem");
    const CERT: &str = include_str!("../../assets/cert.pem");

    #[test]
    fn create_signer() {
        Signer::new(KEY, CERT).unwrap();
    }
}
