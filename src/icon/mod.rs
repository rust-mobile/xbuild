use anyhow::Result;
use image::imageops::FilterType;
use image::io::Reader as ImageReader;
use image::{ImageFormat, RgbaImage};
use std::path::Path;

pub mod android;

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
