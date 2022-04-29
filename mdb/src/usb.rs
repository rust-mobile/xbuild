use anyhow::Result;
use rusb::{
    Device, DeviceHandle, DeviceList, Devices, Direction, GlobalContext, InterfaceDescriptor,
    TransferType, UsbContext,
};
use std::time::Duration;

fn error(err: rusb::Error) -> anyhow::Error {
    if err == rusb::Error::Busy {
        anyhow::anyhow!("device busy, is adb server running? try running `adb kill-server`")
    } else {
        err.into()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Protocol {
    Adb,
    Usbmux,
}

impl Protocol {
    fn new(desc: &InterfaceDescriptor) -> Option<Self> {
        match (
            desc.class_code(),
            desc.sub_class_code(),
            desc.protocol_code(),
        ) {
            (0xff, 0x42, 0x1) => Some(Protocol::Adb),
            (0xff, 0xfe, 0x2) => Some(Protocol::Usbmux),
            _ => None,
        }
    }
}

pub fn usb_devices() -> Result<UsbDeviceList> {
    let context = GlobalContext::default();
    //context.set_log_level(LogLevel::Debug);
    Ok(UsbDeviceList(context.devices()?))
}

pub struct UsbDeviceList(DeviceList<GlobalContext>);

impl UsbDeviceList {
    pub fn iter(&self) -> UsbDevices {
        UsbDevices(self.0.iter())
    }
}

pub struct UsbDevices<'a>(Devices<'a, GlobalContext>);

impl<'a> Iterator for UsbDevices<'a> {
    type Item = Result<UsbDevice>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(device) = self.0.next() {
            if let Some(res) = UsbDevice::new(device).transpose() {
                return Some(res);
            }
        }
        None
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct UsbDevice {
    handle: DeviceHandle<GlobalContext>,
    serial: String,
    protocol: Protocol,
    config: u8,
    iface: u8,
    setting: u8,
    ep_read: u8,
    ep_write: u8,
}

impl UsbDevice {
    fn new(device: Device<GlobalContext>) -> Result<Option<Self>> {
        let device_desc = device.device_descriptor()?;
        let config_desc = device.active_config_descriptor()?;
        for iface in config_desc.interfaces() {
            for iface_desc in iface.descriptors() {
                if let Some(protocol) = Protocol::new(&iface_desc) {
                    let ep_read = iface_desc
                        .endpoint_descriptors()
                        .filter(|ep| ep.transfer_type() == TransferType::Bulk)
                        .filter(|ep| ep.direction() == Direction::In)
                        .map(|ep| ep.address())
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("invalid endpoint"))?;
                    let ep_write = iface_desc
                        .endpoint_descriptors()
                        .filter(|ep| ep.transfer_type() == TransferType::Bulk)
                        .filter(|ep| ep.direction() == Direction::Out)
                        .map(|ep| ep.address())
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("invalid endpoint"))?;
                    let handle = device.open().map_err(error)?;
                    let serial = handle.read_serial_number_string_ascii(&device_desc)?;
                    return Ok(Some(Self {
                        handle,
                        serial,
                        protocol,
                        config: config_desc.number(),
                        iface: iface_desc.interface_number(),
                        setting: iface_desc.setting_number(),
                        ep_read,
                        ep_write,
                    }));
                }
            }
        }
        Ok(None)
    }

    pub fn open(serial: &str, protocol: Protocol) -> Result<Self> {
        let mut device = usb_devices()?
            .iter()
            .filter_map(|res| res.ok())
            .find(|dev| dev.serial == serial && dev.protocol == protocol)
            .ok_or_else(|| anyhow::anyhow!("device with serial {} not found", serial))?;
        device.handle.reset()?;
        device.handle.detach_kernel_driver(device.iface).ok();
        device.handle.claim_interface(device.iface).map_err(error)?;
        device
            .handle
            .set_alternate_setting(device.iface, device.setting)
            .map_err(error)?;
        Ok(device)
    }

    pub fn send(&self, buf: &[u8], timeout: Duration) -> Result<usize> {
        Ok(self.handle.write_bulk(self.ep_write, buf, timeout)?)
    }

    pub fn recv(&self, buf: &mut [u8], timeout: Duration) -> Result<usize> {
        Ok(self.handle.read_bulk(self.ep_read, buf, timeout)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usb_connect() -> Result<()> {
        let devices = usb_devices()?.iter().collect::<Result<Vec<_>>>()?;
        assert_eq!(devices.len(), 1);
        let device = UsbDevice::open(&devices[0].serial, Protocol::Adb)?;
        println!("{:?}", device);
        Ok(())
    }
}
