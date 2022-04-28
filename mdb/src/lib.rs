use anyhow::Result;

mod adb;
mod usb;

pub use crate::adb::AdbConnection;
pub use crate::usb::{usb_devices, Protocol, UsbDevice, UsbDeviceList, UsbDevices};

pub trait Transport<P> {
    fn send(&mut self, packet: P) -> Result<()>;
    fn recv(&mut self) -> Result<P>;
}
