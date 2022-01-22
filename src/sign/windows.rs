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
            49, 34, 4, 32,
            68, 234, 15, 167, 40, 66, 12, 133, 19, 239, 228, 168, 72, 147, 90, 139,
            75, 131, 41, 111, 247, 70, 28, 251, 130, 190, 57, 136, 200, 159, 93, 116,
        ];
        let (rem, res) = der_parser::parse_der(&message_digest).unwrap();
        assert!(rem.is_empty());
        println!("{:#?}", res);
        assert!(false);
    }

    #[test]
    fn decode_sp_indirect_data() {
        let indirect_data = [
            /*160, 130, 1, 8, 48, 130, 1, 4,*/
            48, 53, 6, 10, 43, 6, 1, 4, 1, 130, 55, 2, 1, 30, 48,
            39, 2, 4, 1, 1, 0, 0, 4, 16, 75, 223, 197, 10, 7, 206, 226, 77, 183, 110, 35, 200, 57,
            160, 159, 209, 2, 1, 0, 2, 1, 0, 2, 1, 0, 2, 1, 0, 2, 1, 0, 48, 129, 202, 48, 13, 6, 9,
            96, 134, 72, 1, 101, 3, 4, 2, 1, 5, 0, 4, 129, 184, 65, 80, 80, 88, 65, 88, 80, 67, 37,
            112, 168, 185, 134, 72, 152, 136, 166, 55, 137, 233, 153, 167, 81, 229, 137, 229, 158,
            190, 214, 181, 211, 85, 93, 172, 161, 115, 74, 99, 165, 156, 65, 88, 67, 68, 29, 87,
            205, 69, 139, 92, 201, 89, 248, 232, 221, 244, 67, 120, 231, 192, 229, 135, 200, 178,
            242, 207, 83, 145, 88, 83, 238, 30, 255, 54, 226, 31, 65, 88, 67, 84, 188, 251, 66,
            139, 217, 90, 175, 33, 93, 159, 193, 116, 124, 19, 113, 188, 195, 138, 75, 212, 185,
            133, 87, 115, 195, 93, 4, 189, 198, 152, 59, 190, 65, 88, 66, 77, 228, 142, 202, 253,
            204, 232, 223, 220, 131, 162, 12, 252, 106, 74, 3, 180, 190, 71, 230, 173, 146, 218,
            209, 13, 101, 4, 43, 186, 70, 46, 196, 194, 65, 88, 67, 73, 175, 56, 231, 224, 95, 58,
            68, 216, 201, 155, 33, 50, 3, 124, 19, 157, 149, 107, 194, 174, 170, 108, 34, 110, 128,
            107, 240, 29, 11, 129, 67, 233,
        ];
        use sha2::Digest;
        let dig = sha2::Sha256::digest(&indirect_data);
        println!("{:?}", dig);
        /*let (rem, res) = der_parser::parse_der(&indirect_data).unwrap();
        assert!(rem.is_empty());
        println!("{:#?}", res);*/
        assert!(false);
    }

    #[test]
    fn test_mask() {
        println!("{}", 130 & 0x80);
    }

    #[test]
    fn test_sign() {
        use rsa::{RsaPrivateKey, PaddingScheme, Hash};
        use rsa::pkcs8::FromPrivateKey;
        let digest = [
            68, 234, 15, 167, 40, 66, 12, 133, 19, 239, 228, 168, 72, 147, 90, 139,
            75, 131, 41, 111, 247, 70, 28, 251, 130, 190, 57, 136, 200, 159, 93, 116,
        ];
        const KEY: &str = include_str!("key.pem");
        let signature = b"\x7f\x13uP\xc8m:\x99\xb6\x89u\x85y\xea\xfc\xd8Cw\x96w\x10>j\xa7Z\x8c\xa3\x1f\\\xf4\x82\\\xdf\x8eh;\x10\x16o/\"i\x89\xb9\xf1\x03\x9c\xb0)\x9f\xc4\xfe\xf1\x05\x93\xbeJ\xd2\xeb\xe3\xb1f\xb1rq\x89\xdf\x7f\xe4\xe1\n\xae\xa70\x8c|\xd3\xe6\xe6/\xad\x97\xcb1\xb6\xa0\xf9\x16z\x83R#\xe8n\r\xfdErJ\x01\xfb\xd4\xef\x05\xf9\xab\x08o\x16\xbc)C\xee\x03=$\x88>G\xa4\xba)\xbc\xf4n6\xaa\xfd\xa7e\x15\xb9,|\xd6\xf9\x9b>\xe8\x95\xf7\xc6\x08\n\t\x8a\xd5{j\x8a\xfe{,O\xf3\xd9\x8a\xc79\x9f\x80\xcd\x17k8\xf8\xb3\xc3\x96\xd8\x1a/\xa8\x14R\x14\xaf\x813\x91;>\x99\xd24\x86J\x12\x0e\x89\x0c\xb8?\xfa\xa8\x1dM\x98@vz'\xe6y\xab\xc0\xcb\xc5\xb3\xbeC'$\"\xd2\x15\xaf0\xa3\x05\xcbj\x18j\x11\xa2\xfd\xe7\xe6y\xcf\xadd\x99\xa9\xdc\xc4\xc2`\x1d\xb0\xe3\xdb\xfeC\xdc\xce\xe5@\xde;P\xfav\x8c\xff";
        let key = RsaPrivateKey::from_pkcs8_pem(KEY).unwrap();
        let padding = PaddingScheme::new_pkcs1v15_sign(Some(Hash::SHA2_256));
        let sig = key.sign(padding, &digest).unwrap();
        assert_eq!(sig.len(), signature.len());
        assert_eq!(sig, signature);
    }
}
