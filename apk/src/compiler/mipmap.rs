use crate::res::Chunk;
use anyhow::Result;
use std::path::Path;
use xcommon::Scaler;

const DPI_SIZE: [u32; 5] = [48, 72, 96, 144, 192];

pub fn compile_mipmap(icon: &Path) -> Result<(Chunk, u32)> {
    let mut scaler = Scaler::open(icon)?;
    scaler.optimize();
    Ok((Chunk::Null, 0))
}
