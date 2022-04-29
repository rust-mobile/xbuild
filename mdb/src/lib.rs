use anyhow::Result;
use rsa::pkcs8::DecodePrivateKey;
use rsa::RsaPrivateKey;

mod adb;
mod usb;

pub use crate::adb::AdbConnection;
pub use crate::usb::{usb_devices, Protocol, UsbDevice, UsbDeviceList, UsbDevices};

pub trait Transport<P> {
    fn send(&mut self, packet: P) -> Result<()>;
    fn recv(&mut self) -> Result<P>;
}

pub fn adbkey() -> Result<RsaPrivateKey> {
    let home = dirs::home_dir().unwrap();
    Ok(RsaPrivateKey::read_pkcs8_pem_file(
        &home.join(".android/adbkey"),
    )?)
}
