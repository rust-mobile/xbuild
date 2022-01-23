use anyhow::Result;
use byteorder::{BigEndian, ReadBytesExt};
use rasn_cms::{ContentInfo, SignedData, CONTENT_SIGNED_DATA};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

const P7X_MAGIC: u32 = 0x504b4358;

pub fn read_p7x(path: &Path) -> Result<()> {
    let f = File::open(path)?;
    let mut r = BufReader::new(f);
    let magic = r.read_u32::<BigEndian>()?;
    if magic != P7X_MAGIC {
        anyhow::bail!("not a valid p7x file");
    }
    let mut der = vec![];
    r.read_to_end(&mut der)?;
    let info = rasn::der::decode::<ContentInfo>(&der).map_err(|err| anyhow::anyhow!("{}", err))?;
    anyhow::ensure!(CONTENT_SIGNED_DATA == info.content_type);
    let data = rasn::der::decode::<SignedData>(info.content.as_bytes())
        .map_err(|err| anyhow::anyhow!("{}", err))?;
    println!("{:#?}", data);
    Ok(())
}

//#[derive(rasn::Type)]
pub struct SpcSipinfo {
    i1: u32,
    s1: [u8; 16],
    i2: u32,
    i3: u32,
    i4: u32,
    i5: u32,
    i6: u32,
}

impl Default for SpcSipinfo {
    fn default() -> Self {
        const SPC_SIPINFO_MAGIC_INT: u32 = 0x0101_0000;
        const SPC_SIPINFO_MAGIC: [u8; 16] = [
            0x4b, 0xdf, 0xc5, 0x0a, 0x07, 0xce, 0xe2, 0x4d, 0xb7, 0x6e, 0x23, 0xc8, 0x39, 0xa0,
            0x9f, 0xd1,
        ];
        Self {
            i1: SPC_SIPINFO_MAGIC_INT,
            s1: SPC_SIPINFO_MAGIC,
            i2: 0,
            i3: 0,
            i4: 0,
            i5: 0,
            i6: 0,
        }
    }
}

/*
        pub struct SpcIndirectData {}

        let encap_content_info = EncapsulatedContentInfo {
            content_type: SPC_INDIRECT_DATA_OBJID.into(),
            // class ContextSpecific, raw_tag: 160
            content: Any::new(vec![]),
        };
*/
