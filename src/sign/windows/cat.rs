use anyhow::Result;
use rasn_cms::{ContentInfo, SignedData, CONTENT_SIGNED_DATA};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

pub fn read_cms(path: &Path) -> Result<()> {
    let f = File::open(path)?;
    let mut r = BufReader::new(f);
    let mut der = vec![];
    r.read_to_end(&mut der)?;
    let info = rasn::der::decode::<ContentInfo>(&der).map_err(|err| anyhow::anyhow!("{}", err))?;
    anyhow::ensure!(CONTENT_SIGNED_DATA == info.content_type);
    let data = rasn::der::decode::<SignedData>(info.content.as_bytes())
        .map_err(|err| anyhow::anyhow!("{}", err))?;
    println!("{:#?}", data);
    Ok(())
}
