use anyhow::Result;
use rsa::pkcs1::FromRsaPrivateKey;
use rsa::RsaPrivateKey;

pub mod android;

pub struct Signer {
    key: RsaPrivateKey,
}

impl Signer {
    pub fn new(private_key: &str) -> Result<Self> {
        let key = RsaPrivateKey::from_pkcs1_pem(private_key)?;
        Ok(Self { key })
    }
}
