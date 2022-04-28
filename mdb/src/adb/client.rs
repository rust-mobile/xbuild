use crate::adb::packet::{AdbPacket, Command};
use crate::adb::{AdbTcpTransport, AdbUsbTransport};
use crate::Transport;
use anyhow::Result;
use rsa::{Hash, PaddingScheme, RsaPrivateKey, RsaPublicKey};
use std::net::ToSocketAddrs;

const VERSION: u32 = 0x0100_0000;
const MAX_DATA: u32 = 0x10_0000;

pub struct AdbConnection<T> {
    transport: T,
}

impl AdbConnection<AdbUsbTransport> {
    pub fn usb(private_key: &RsaPrivateKey, serial: &str) -> Result<Self> {
        let transport = AdbUsbTransport::connect(serial)?;
        Self::new(transport, VERSION, MAX_DATA, "host::", private_key)
    }
}

impl AdbConnection<AdbTcpTransport> {
    pub fn tcp(private_key: &RsaPrivateKey, addrs: impl ToSocketAddrs) -> Result<Self> {
        let transport = AdbTcpTransport::connect(addrs)?;
        Self::new(transport, VERSION, MAX_DATA, "host::", private_key)
    }
}

impl<T: Transport<AdbPacket>> AdbConnection<T> {
    pub fn new(
        mut transport: T,
        version: u32,
        max_data: u32,
        system_identity: &str,
        private_key: &RsaPrivateKey,
    ) -> Result<Self> {
        transport.send(AdbPacket::connect(version, max_data, system_identity))?;
        let mut auth = 0;
        loop {
            let packet = transport.recv()?;
            match packet.command() {
                Command::Connect => {
                    let device_id = String::from_utf8_lossy(packet.payload());
                    log::debug!(
                        "handshake ok: device_id = {}, version = 0x{:x}, max_data = 0x{:x}",
                        device_id,
                        packet.arg0(),
                        packet.arg1(),
                    );
                    break;
                }
                Command::Auth => match auth {
                    0 => {
                        let padding = PaddingScheme::new_pkcs1v15_sign(Some(Hash::SHA1));
                        let signature = private_key.sign(padding, packet.payload())?;
                        transport.send(AdbPacket::auth_signature(signature))?;
                        auth += 1;
                    }
                    1 => {
                        let public_key = RsaPublicKey::from(private_key);
                        transport.send(AdbPacket::auth_rsa_public_key(public_key))?;
                        auth += 2;
                    }
                    _ => {
                        anyhow::bail!("authentication failed");
                    }
                },
                cmd => {
                    anyhow::bail!("unexpected command {:?}", cmd);
                }
            }
        }

        Ok(Self { transport })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::pkcs8::DecodePrivateKey;

    #[test]
    fn test_client_tcp() -> Result<()> {
        env_logger::try_init().ok();
        let private_key = RsaPrivateKey::read_pkcs8_pem_file("/home/dvc/.android/adbkey")?;
        let _conn = AdbConnection::tcp(&private_key, "192.168.2.43:5555")?;
        Ok(())
    }

    #[test]
    fn test_client_usb() -> Result<()> {
        env_logger::try_init().ok();
        let private_key = RsaPrivateKey::read_pkcs8_pem_file("/home/dvc/.android/adbkey")?;
        let _conn = AdbConnection::usb(&private_key, "16ee50bc")?;
        Ok(())
    }
}
