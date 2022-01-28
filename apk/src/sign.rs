use anyhow::Result;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use rasn_pkix::Certificate;
use rsa::pkcs8::FromPublicKey;
use rsa::{Hash, PaddingScheme, PublicKey, RsaPublicKey};
use sha2::{Digest as _, Sha256};
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::Path;
use xcommon::Signer;

const DEBUG_KEY_PEM: &str = include_str!("../assets/debug.key.pem");
const DEBUG_CERT_PEM: &str = include_str!("../assets/debug.cert.pem");

const CENTRAL_DIRECTORY_END_SIGNATURE: u32 = 0x06054b50;
const APK_SIGNING_BLOCK_MAGIC: &[u8] = b"APK Sig Block 42";
const APK_SIGNING_BLOCK_V2_ID: u32 = 0x7109871a;
const APK_SIGNING_BLOCK_V3_ID: u32 = 0xf05368c0;
const APK_SIGNING_BLOCK_V4_ID: u32 = 0x42726577;
const RSA_PKCS1V15_SHA2_256: u32 = 0x0103;
const MAX_CHUNK_SIZE: usize = 1024 * 1024;

pub fn verify(path: &Path) -> Result<Vec<Certificate>> {
    let f = File::open(path)?;
    let mut r = BufReader::new(f);
    let sblock = parse_apk_signing_block(&mut r)?;
    let mut sblockv2 = None;
    for block in &sblock.blocks {
        match block.id {
            APK_SIGNING_BLOCK_V2_ID => {
                tracing::debug!("v2 signing block");
                sblockv2 = Some(*block);
            }
            APK_SIGNING_BLOCK_V3_ID => {
                tracing::debug!("v3 signing block");
            }
            APK_SIGNING_BLOCK_V4_ID => {
                tracing::debug!("v4 signing block");
            }
            id => {
                tracing::debug!("unknown signing block 0x{:x}", id);
            }
        }
    }
    let block = if let Some(block) = sblockv2 {
        parse_apk_signing_block_v2(&mut r, block)?
    } else {
        anyhow::bail!("no sigining block v2 found");
    };
    let zip_hash = compute_digest(&mut r, sblock.sb_start, sblock.cd_start, sblock.cde_start)?;
    let mut certificates = vec![];
    for signer in &block.signers {
        if signer.signatures.is_empty() {
            anyhow::bail!("found no signatures in v2 block");
        }
        for sig in &signer.signatures {
            if sig.algorithm != RSA_PKCS1V15_SHA2_256 {
                anyhow::bail!(
                    "found unsupported signature algorithm 0x{:x}",
                    sig.algorithm
                );
            }
            let pubkey = RsaPublicKey::from_public_key_der(&signer.public_key)?;
            let digest = Sha256::digest(&signer.signed_data);
            let padding = PaddingScheme::new_pkcs1v15_sign(Some(Hash::SHA2_256));
            pubkey.verify(padding, &digest, &sig.signature)?;
        }
        let mut r = Cursor::new(&signer.signed_data[..]);
        let signed_data = parse_signed_data(&mut r)?;
        if signed_data.digests.is_empty() {
            anyhow::bail!("found no digests in v2 block");
        }
        for digest in &signed_data.digests {
            if digest.algorithm != RSA_PKCS1V15_SHA2_256 {
                anyhow::bail!(
                    "found unsupported digest algorithm 0x{:x}",
                    digest.algorithm
                );
            }
            if digest.digest != zip_hash {
                anyhow::bail!("computed hash doesn't match signed hash.");
            }
        }
        for cert in &signed_data.certificates {
            let cert =
                rasn::der::decode::<Certificate>(cert).map_err(|err| anyhow::anyhow!("{}", err))?;
            certificates.push(cert);
        }
        for attr in &signed_data.additional_attributes {
            tracing::debug!("v2: additional attribute: 0x{:x} {:?}", attr.0, &attr.1);
        }
    }
    Ok(certificates)
}

pub fn sign(path: &Path, signer: Option<Signer>) -> Result<()> {
    let _singer = signer
        .map(Ok)
        .unwrap_or_else(|| Signer::new(DEBUG_KEY_PEM, DEBUG_CERT_PEM))?;
    todo!();
}

fn compute_digest<R: Read + Seek>(
    r: &mut R,
    sb_start: u64,
    cd_start: u64,
    cde_start: u64,
) -> Result<[u8; 32]> {
    let mut chunks = vec![];
    let mut hasher = Sha256::new();
    let mut chunk = vec![0u8; MAX_CHUNK_SIZE];

    // chunk contents
    let mut pos = r.seek(SeekFrom::Start(0))?;
    while pos < sb_start {
        hash_chunk(&mut chunks, r, sb_start, &mut hasher, &mut chunk, &mut pos)?;
    }

    // chunk cd
    let mut pos = r.seek(SeekFrom::Start(cd_start))?;
    while pos < cde_start {
        hash_chunk(&mut chunks, r, cde_start, &mut hasher, &mut chunk, &mut pos)?;
    }

    // chunk cde
    chunk.clear();
    r.read_to_end(&mut chunk)?;
    let mut cursor = Cursor::new(&mut chunk);
    cursor.seek(SeekFrom::Start(16))?;
    cursor.write_u32::<LittleEndian>(sb_start as u32)?;
    hasher.update(&[0xa5]);
    assert!(chunk.len() <= MAX_CHUNK_SIZE);
    hasher.update(&(chunk.len() as u32).to_le_bytes());
    hasher.update(&chunk);
    chunks.push(hasher.finalize_reset().into());

    // compute root
    hasher.update(&[0x5a]);
    hasher.update(&(chunks.len() as u32).to_le_bytes());
    for chunk in &chunks {
        hasher.update(&chunk);
    }
    Ok(hasher.finalize().into())
}

fn hash_chunk<R: Read + Seek>(
    chunks: &mut Vec<[u8; 32]>,
    r: &mut R,
    size: u64,
    hasher: &mut Sha256,
    buffer: &mut Vec<u8>,
    pos: &mut u64,
) -> Result<()> {
    let end = std::cmp::min(*pos + MAX_CHUNK_SIZE as u64, size);
    let len = (end - *pos) as usize;
    buffer.resize(len, 0);
    r.read_exact(buffer).unwrap();
    hasher.update(&[0xa5]);
    hasher.update(&(len as u32).to_le_bytes());
    hasher.update(&buffer);
    chunks.push(hasher.finalize_reset().into());
    *pos = end;
    Ok(())
}

#[derive(Debug, Default)]
pub struct SignedData {
    pub digests: Vec<Digest>,
    pub certificates: Vec<Vec<u8>>,
    pub additional_attributes: Vec<(u32, Vec<u8>)>,
}

#[derive(Debug, Default)]
pub struct Digest {
    pub algorithm: u32,
    pub digest: Vec<u8>,
}

fn parse_signed_data<R: Read + Seek>(r: &mut R) -> Result<SignedData> {
    let mut signed_data = SignedData::default();
    let mut remaining_digests_size = r.read_u32::<LittleEndian>()?;
    while remaining_digests_size > 0 {
        let digest_size = r.read_u32::<LittleEndian>()?;
        let algorithm = r.read_u32::<LittleEndian>()?;
        let size = r.read_u32::<LittleEndian>()?;
        let mut digest = vec![0; size as usize as _];
        r.read_exact(&mut digest)?;
        signed_data.digests.push(Digest { algorithm, digest });
        remaining_digests_size -= digest_size + 4;
    }
    let mut remaining_certificates_size = r.read_u32::<LittleEndian>()?;
    while remaining_certificates_size > 0 {
        let length = r.read_u32::<LittleEndian>()?;
        let mut cert = vec![0; length as usize];
        r.read_exact(&mut cert)?;
        signed_data.certificates.push(cert);
        remaining_certificates_size -= length + 4;
    }
    let mut remaining_additional_attributes_size = r.read_u32::<LittleEndian>()?;
    while remaining_additional_attributes_size > 0 {
        let length = r.read_u32::<LittleEndian>()?;
        let id = r.read_u32::<LittleEndian>()?;
        let mut value = vec![0; length as usize - 4];
        r.read_exact(&mut value)?;
        signed_data.additional_attributes.push((id, value));
        remaining_additional_attributes_size -= length + 4;
    }
    Ok(signed_data)
}

#[derive(Debug)]
pub struct ApkSignatureBlockV2 {
    pub signers: Vec<ApkSigner>,
}

#[derive(Debug)]
pub struct ApkSigner {
    pub signed_data: Vec<u8>,
    pub signatures: Vec<ApkSignature>,
    pub public_key: Vec<u8>,
}

#[derive(Debug)]
pub struct ApkSignature {
    pub algorithm: u32,
    pub signature: Vec<u8>,
}

fn parse_apk_signing_block_v2<R: Read + Seek>(
    r: &mut R,
    block: ApkOpaqueBlock,
) -> Result<ApkSignatureBlockV2> {
    r.seek(SeekFrom::Start(block.start))?;
    let mut signers = vec![];
    let mut remaining_size = r.read_u32::<LittleEndian>()? as u64;
    while remaining_size > 0 {
        let signer_size = r.read_u32::<LittleEndian>()?;

        let signed_data_size = r.read_u32::<LittleEndian>()?;
        let mut signed_data = vec![0; signed_data_size as _];
        r.read_exact(&mut signed_data)?;

        let mut signatures = vec![];
        let mut remaining_signature_size = r.read_u32::<LittleEndian>()?;
        while remaining_signature_size > 0 {
            let signature_size = r.read_u32::<LittleEndian>()?;
            let algorithm = r.read_u32::<LittleEndian>()?;
            let size = r.read_u32::<LittleEndian>()?;
            let mut signature = vec![0; size as usize];
            r.read_exact(&mut signature)?;
            signatures.push(ApkSignature {
                algorithm,
                signature,
            });
            remaining_signature_size -= signature_size + 4;
        }

        let public_key_size = r.read_u32::<LittleEndian>()?;
        let mut public_key = vec![0; public_key_size as _];
        r.read_exact(&mut public_key)?;

        signers.push(ApkSigner {
            signed_data,
            signatures,
            public_key,
        });
        remaining_size -= signer_size as u64 + 4;
    }
    Ok(ApkSignatureBlockV2 { signers })
}

#[derive(Debug, Default)]
pub struct ApkSignatureBlock {
    pub blocks: Vec<ApkOpaqueBlock>,
    pub sb_start: u64,
    pub cd_start: u64,
    pub cde_start: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct ApkOpaqueBlock {
    pub id: u32,
    pub start: u64,
    pub len: u64,
}

fn parse_apk_signing_block<R: Read + Seek>(r: &mut R) -> Result<ApkSignatureBlock> {
    let mut block = ApkSignatureBlock::default();
    block.cde_start = find_cde_start_pos(r)?;
    r.seek(SeekFrom::Start(block.cde_start + 16))?;
    block.cd_start = r.read_u32::<LittleEndian>()? as u64;
    r.seek(SeekFrom::Start(block.cd_start - 16 - 8))?;
    let mut remaining_size = r.read_u64::<LittleEndian>()?;
    let mut magic = [0; 16];
    r.read_exact(&mut magic)?;
    if magic != APK_SIGNING_BLOCK_MAGIC {
        block.sb_start = block.cd_start;
        return Ok(block);
    }
    let mut pos = r.seek(SeekFrom::Current(-(remaining_size as i64)))?;
    block.sb_start = pos - 8;
    while remaining_size > 24 {
        let length = r.read_u64::<LittleEndian>()?;
        let id = r.read_u32::<LittleEndian>()?;
        block.blocks.push(ApkOpaqueBlock {
            id,
            start: pos + 8 + 4,
            len: length - 4,
        });
        pos = r.seek(SeekFrom::Start(pos + length + 8))?;
        remaining_size -= length + 8;
    }
    Ok(block)
}

// adapted from zip-rs
fn find_cde_start_pos<R: Read + Seek>(reader: &mut R) -> Result<u64> {
    const HEADER_SIZE: u64 = 22;
    const BYTES_BETWEEN_MAGIC_AND_COMMENT_SIZE: u64 = HEADER_SIZE - 6;
    let file_length = reader.seek(SeekFrom::End(0))?;
    let search_upper_bound = file_length.saturating_sub(HEADER_SIZE + ::std::u16::MAX as u64);
    if file_length < HEADER_SIZE {
        anyhow::bail!("Invalid zip header");
    }
    let mut pos = file_length - HEADER_SIZE;
    while pos >= search_upper_bound {
        reader.seek(SeekFrom::Start(pos as u64))?;
        if reader.read_u32::<LittleEndian>()? == CENTRAL_DIRECTORY_END_SIGNATURE {
            reader.seek(SeekFrom::Current(
                BYTES_BETWEEN_MAGIC_AND_COMMENT_SIZE as i64,
            ))?;
            return Ok(pos);
        }
        pos = match pos.checked_sub(1) {
            Some(p) => p,
            None => break,
        };
    }
    anyhow::bail!("Could not find central directory end");
}
