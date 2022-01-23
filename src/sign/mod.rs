use anyhow::Result;
use rasn_pkix::Certificate;
use rsa::pkcs8::FromPrivateKey;
use rsa::{Hash, PaddingScheme, RsaPrivateKey};

pub mod android;
pub mod windows;

pub struct Signer {
    key: RsaPrivateKey,
    cert: Certificate,
}

impl Signer {
    /// Creates a new signer using a private key and a certificate.
    ///
    /// A new self signed certificate can be generated using openssl:
    /// ```sh
    /// openssl req -newkey rsa:2048 -new -nodes -x509 -days 3650 -keyout key.pem -out cert.pem
    /// ```
    pub fn new(private_key: &str, certificate: &str) -> Result<Self> {
        let key = RsaPrivateKey::from_pkcs8_pem(private_key)?;
        let pem = pem::parse(certificate)?;
        anyhow::ensure!(pem.tag == "CERTIFICATE");
        let cert = rasn::der::decode::<Certificate>(&pem.contents)
            .map_err(|err| anyhow::anyhow!("{}", err))?;
        Ok(Self { key, cert })
    }

    pub fn sign(&self, digest: [u8; 32]) -> Vec<u8> {
        let padding = PaddingScheme::new_pkcs1v15_sign(Some(Hash::SHA2_256));
        self.key.sign(padding, &digest).unwrap()
    }

    pub fn cert(&self) -> &Certificate {
        &self.cert
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = include_str!("key.pem");
    const CERT: &str = include_str!("cert.pem");

    #[test]
    fn create_signer() {
        Signer::new(KEY, CERT).unwrap();
    }
}
