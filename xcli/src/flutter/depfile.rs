use anyhow::Result;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub fn depfile_is_dirty(path: &Path) -> Result<bool> {
    let mut f = BufReader::new(File::open(path)?);
    let mut buffer = Vec::with_capacity(256);
    let n = f.read_until(' ' as u8, &mut buffer)?;
    if n == 0 {
        return Ok(true);
    }
    let len = buffer.len();
    if buffer[len - 2] != ':' as u8 {
        let target = std::str::from_utf8(&buffer)?;
        anyhow::bail!(
            "invalid depfile: target `{}` doesn't end with a `:`",
            target
        );
    }
    let target = Path::new(std::str::from_utf8(&buffer[..(len - 2)])?);
    if !target.exists() {
        return Ok(true);
    }
    let modtime = std::fs::metadata(&target)?.modified()?;
    loop {
        buffer.clear();
        let n = f.read_until(' ' as u8, &mut buffer)?;
        if n == 0 {
            break;
        }
        let dep = Path::new(std::str::from_utf8(&buffer[..(n - 1)])?);
        if !dep.exists() {
            return Ok(true);
        }
        let dep_modtime = std::fs::metadata(dep)?.modified()?;
        if dep_modtime > modtime {
            return Ok(true);
        }
    }
    Ok(false)
}
