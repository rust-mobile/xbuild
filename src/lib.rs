use anyhow::Result;
use image::imageops::FilterType;
use image::io::Reader as ImageReader;
use image::{ImageFormat, RgbaImage};
use rasn_pkix::Certificate;
use rsa::pkcs8::FromPrivateKey;
use rsa::{Hash, PaddingScheme, RsaPrivateKey};
use std::path::Path;
use zip::CompressionMethod;

pub mod apk;
pub mod msix;

pub struct Scaler {
    img: RgbaImage,
}

impl Scaler {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let img = ImageReader::open(path)?.decode()?.to_rgba8();
        let (width, height) = img.dimensions();
        if width != height {
            anyhow::bail!("expected width == height");
        }
        if width < 512 {
            anyhow::bail!("expected icon of at least 512x512 px");
        }
        Ok(Self { img })
    }

    pub fn write<P: AsRef<Path>>(&self, path: P, size: u32) -> Result<()> {
        let path = path.as_ref();
        image::imageops::resize(&self.img, size, size, FilterType::Triangle)
            .save_with_format(path, ImageFormat::Png)?;
        Ok(())
    }
}

pub struct Signer {
    key: RsaPrivateKey,
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
        let pem = pem::parse(certificate)?;
        anyhow::ensure!(pem.tag == "CERTIFICATE");
        let cert = rasn::der::decode::<Certificate>(&pem.contents)
            .map_err(|err| anyhow::anyhow!("{}", err))?;
        Ok(Self { key, cert })
    }

    pub fn sign(&self, digest: [u8; 32]) -> Vec<u8> {
        let padding = PaddingScheme::new_pkcs1v15_sign(Some(Hash::SHA2_256));
        self.key.sign(padding, &digest).unwrap()
    }

    pub fn cert(&self) -> &Certificate {
        &self.cert
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ZipFileOptions {
    Unaligned,
    Aligned(u16),
    Compressed,
}

impl ZipFileOptions {
    fn alignment(self) -> u16 {
        match self {
            Self::Aligned(align) => align,
            _ => 1,
        }
    }

    fn compression_method(&self) -> CompressionMethod {
        match self {
            Self::Compressed => CompressionMethod::Deflated,
            _ => CompressionMethod::Stored,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = include_str!("../assets/key.pem");
    const CERT: &str = include_str!("../assets/cert.pem");

    #[test]
    fn create_signer() {
        Signer::new(KEY, CERT).unwrap();
    }
}
