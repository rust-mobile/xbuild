use anyhow::Result;
use image::imageops::FilterType;
use image::io::Reader as ImageReader;
use image::{DynamicImage, GenericImageView, ImageOutputFormat};
use rsa::pkcs8::FromPrivateKey;
use rsa::{Hash, PaddingScheme, RsaPrivateKey, RsaPublicKey};
use sha2::{Digest, Sha256};
use std::io::{Seek, Write};
use std::path::Path;
use zip::CompressionMethod;

pub use rasn_pkix::Certificate;

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
