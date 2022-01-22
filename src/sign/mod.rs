use anyhow::Result;
use rasn::prelude::*;
use rasn_cms::{
    AlgorithmIdentifier, EncapsulatedContentInfo, IssuerAndSerialNumber, SignedData,
    SignerIdentifier, SignerInfo,
};
use rasn_pkix::{Attribute, Certificate};
use rsa::pkcs8::FromPrivateKey;
use rsa::RsaPrivateKey;

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

    pub fn sign_pkcs7(
        &self,
        digests: &[[u8; 32]; 5],
        encap_content_info: EncapsulatedContentInfo,
        digest: [u8; 32],
        signature: Vec<u8>,
        cert: &Certificate,
    ) -> SignedData {
        const SPC_INDIRECT_DATA_OBJID: ConstOid = ConstOid(&[1, 3, 6, 1, 4, 1, 311, 2, 1, 4]);
        const SPC_SP_OPUS_INFO_OBJID: ConstOid = ConstOid(&[1, 3, 6, 1, 4, 1, 311, 2, 1, 12]);
        const SZOID_CTL: ConstOid = ConstOid(&[1, 3, 6, 2, 4, 1, 311, 10, 1]);

        /*let encap_content_info = EncapsulatedContentInfo {
            content_type: SPC_INDIRECT_DATA_OBJID.into(),
            // class ContextSpecific, raw_tag: 160
            content: Any::new(vec![]),
        };*/
        //let digest = Sha256::digest(encap_content_info.contents.as_bytes()[..8]);

        let digest_algorithm = AlgorithmIdentifier {
            algorithm:
                Oid::JOINT_ISO_ITU_T_COUNTRY_US_ORGANIZATION_GOV_CSOR_NIST_ALGORITHMS_HASH_SHA256
                    .into(),
            parameters: Some(Any::new(vec![5, 0])),
        };
        let signer_info = SignerInfo {
            version: 1.into(),
            sid: SignerIdentifier::IssuerAndSerialNumber(IssuerAndSerialNumber {
                issuer: cert.tbs_certificate.issuer.clone(),
                serial_number: cert.tbs_certificate.serial_number.clone(),
            }),
            digest_algorithm: digest_algorithm.clone(),
            signed_attrs: Some({
                let mut signed_attrs = SetOf::default();
                signed_attrs.insert(Attribute {
                    r#type: Oid::ISO_MEMBER_BODY_US_RSADSI_PKCS9_CONTENT_TYPE.into(),
                    value: {
                        let mut content_type = SetOf::default();
                        content_type.insert(ObjectIdentifier::from(SPC_INDIRECT_DATA_OBJID));
                        Any::new(rasn::der::encode(&content_type).unwrap())
                    },
                });
                signed_attrs.insert(Attribute {
                    r#type: Oid::ISO_MEMBER_BODY_US_RSADSI_PKCS9_MESSAGE_DIGEST.into(),
                    value: {
                        let mut digests = SetOf::default();
                        digests.insert(OctetString::from(digest.to_vec()));
                        Any::new(rasn::der::encode(&digests).unwrap())
                    },
                });
                // TODO: is this needed?
                signed_attrs.insert(Attribute {
                    r#type: SPC_SP_OPUS_INFO_OBJID.into(),
                    value: {
                        let mut info = SetOf::default();
                        info.insert(SequenceOf::<()>::default());
                        Any::new(rasn::der::encode(&info).unwrap())
                    },
                });
                signed_attrs
            }),
            signature_algorithm: AlgorithmIdentifier {
                algorithm: Oid::ISO_MEMBER_BODY_US_RSADSI_PKCS1.into(),
                parameters: Some(Any::new(vec![5, 0])),
            },
            signature: OctetString::from(signature.to_vec()),
            unsigned_attrs: Some({
                let mut unsigned_attrs = SetOf::default();
                // TODO: 1.3.6.1.4.1.311.3.3.1 timestamp? optional?
                unsigned_attrs
            }),
        };
        SignedData {
            version: 1.into(),
            digest_algorithms: {
                let mut digest_algorithms = SetOf::default();
                digest_algorithms.insert(digest_algorithm);
                digest_algorithms
            },
            encap_content_info,
            certificates: Some(SetOf::default()),
            crls: None,
            signer_infos: {
                let mut signer_infos = SetOf::default();
                signer_infos.insert(signer_info);
                signer_infos
            },
        }
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
