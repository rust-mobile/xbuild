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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_pkcs9_content_type() {
        let content_type = [49, 12, 6, 10, 43, 6, 1, 4, 1, 130, 55, 2, 1, 4];
        let (rem, res) = der_parser::parse_der(&content_type).unwrap();
        assert!(rem.is_empty());
        println!("{:#?}", res);
        assert!(false);
    }

    #[test]
    fn decode_opus_info() {
        let opus_info = [49, 2, 48, 0];
        let (rem, res) = der_parser::parse_der(&opus_info).unwrap();
        assert!(rem.is_empty());
        println!("{:#?}", res);
        assert!(false);
    }

    #[test]
    fn decode_pkcs9_message_digest() {
        let message_digest = [
            49, 34, 4, 32, 68, 234, 15, 167, 40, 66, 12, 133, 19, 239, 228, 168, 72, 147, 90, 139,
            75, 131, 41, 111, 247, 70, 28, 251, 130, 190, 57, 136, 200, 159, 93, 116,
        ];
        let (rem, res) = der_parser::parse_der(&message_digest).unwrap();
        assert!(rem.is_empty());
        println!("{:#?}", res);
        assert!(false);
    }
}
