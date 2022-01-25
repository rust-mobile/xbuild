use crate::Scaler;
use anyhow::Result;
use std::path::Path;

pub const DPI_LABEL: [&str; 5] = ["mdpi", "hdpi", "xhdpi", "xxhdpi", "xxxhdpi"];

pub const DPI_SIZE: [u32; 5] = [48, 72, 96, 144, 192];

pub fn mipmap_ic_launcher<P: AsRef<Path>>(icon: P, res: P) -> Result<()> {
    let scaler = Scaler::open(icon)?;
    for (label, size) in DPI_LABEL.iter().zip(DPI_SIZE) {
        let path = res.as_ref().join(format!("mipmap-{}", label));
        std::fs::create_dir_all(&path)?;
        let path = path.join("ic_launcher.png");
        scaler.write(path, size)?;
    }
    Ok(())
}
