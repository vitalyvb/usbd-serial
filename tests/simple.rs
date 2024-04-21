#![allow(unused_variables)]

use usbd_class_tester::prelude::*;

use usb_device::bus::UsbBusAllocator;
use usbd_serial::SerialPort;

struct MkSerial {}

impl UsbDeviceCtx for MkSerial {
    type C<'c> = SerialPort<'c, EmulatedUsbBus>;

    fn create_class<'a>(
        &mut self,
        alloc: &'a UsbBusAllocator<EmulatedUsbBus>,
    ) -> AnyResult<Self::C<'a>> {
        Ok(SerialPort::new(&alloc))
    }
}

#[test]
fn test_descriptors() {
    MkSerial {}
        .with_usb(|mut ser, mut dev| {
            let vec = dev
                .device_get_descriptor(&mut ser, 2, 0, 0, 255)
                .expect("vec");

            let device = &vec[..9];
            let interf = &vec[9..18];
            let cs = &vec[18..37];
            let ep1 = &vec[37..44];
            let interf2 = &vec[44..53];
            let ep2 = &vec[53..60];
            let ep3 = &vec[60..67];

            // device descriptor
            assert_eq!(
                device,
                &[
                    9, // bLength
                    2, // bDescriptorType = 2
                    67, 0,   // wTotalLength
                    2,   // bNumInterfaces
                    1,   // bConfigurationValue
                    0,   // iConfiguration
                    192, // bmAttributes
                    125  // bMaxPower
                ]
            );

            // interface descriptor
            assert_eq!(
                interf,
                &[
                    9, // bLength
                    4, // bDescriptorType = 4
                    0, // bInterfaceNumber
                    0, // bAlternateSetting
                    1, // bNumEndpoints
                    2, // bInterfaceClass = 2 CDC
                    2, // bInterfaceSubClass = 2 ACM
                    0, // bInterfaceProtocol = 0
                    0  // iInterface
                ]
            );

            // endpoint descriptor
            assert_eq!(
                ep1,
                &[
                    7,   // bLength
                    5,   // bDescriptorType = 5
                    129, // bEndpointAddress
                    3,   // bmAttributes
                    8, 0,   // wMaxPacketSize
                    255, // bInterval
                ]
            );

            // alt interface descriptor
            assert_eq!(
                interf2,
                &[
                    9,  // bLength
                    4,  // bDescriptorType = 4
                    1,  // bInterfaceNumber
                    0,  // bAlternateSetting
                    2,  // bNumEndpoints
                    10, // bInterfaceClass = 10 Data
                    0,  // bInterfaceSubClass = 0
                    0,  // bInterfaceProtocol = 0
                    0   // iInterface
                ]
            );

            // endpoint descriptor
            assert_eq!(
                ep2,
                &[
                    7,   // bLength
                    5,   // bDescriptorType = 5
                    130, // bEndpointAddress
                    2,   // bmAttributes
                    64, 0, // wMaxPacketSize
                    0, // bInterval
                ]
            );

            // endpoint descriptor
            assert_eq!(
                ep3,
                &[
                    7, // bLength
                    5, // bDescriptorType = 5
                    1, // bEndpointAddress
                    2, // bmAttributes
                    64, 0, // wMaxPacketSize
                    0, // bInterval
                ]
            );
        })
        .expect("with_usb");
}

#[test]
fn test_short_write_read() {
    MkSerial {}
        .with_usb(|mut ser, mut dev| {
            let mut buf = [0u8; 1024];

            let len = ser.write(&[1, 2, 3]).expect("len");
            assert_eq!(len, 3);

            let vec = dev.ep_read(&mut ser, 2, 1024).expect("vec");
            assert_eq!(vec, [1, 2, 3]);

            let vec = dev.ep_read(&mut ser, 2, 1024).expect("vec");
            assert_eq!(vec, []);

            let len = dev.ep_write(&mut ser, 1, &[4, 5, 6, 7]).expect("len");
            assert_eq!(len, 4);

            let len = ser.read(&mut buf).expect("len");
            assert_eq!(len, 4);
            assert_eq!(buf[0..4], [4, 5, 6, 7]);
        })
        .expect("with_usb");
}

#[test]
fn test_long_read1() {
    MkSerial {}
        .with_usb(|mut ser, mut dev| {
            let data: &[u8] = &(0..256)
                .flat_map(|a| (a as u8).to_le_bytes())
                .collect::<Vec<_>>();
            assert_eq!(data.len(), 256);

            let mut buf = [0u8; 1024];

            let len = dev.ep_write(&mut ser, 1, &data[0..129]).expect("len");
            assert_eq!(len, 129);

            // Serial pull data from the endpoint buffer
            // only when read() is called, simply polling
            // is not enough.
            //
            // Pull data from endpoint buffer to serial buffer
            // and preserve all data in the buffer
            let _ = ser.read(&mut []);

            let len = ser.read(&mut buf).expect("len");
            assert_eq!(len, 128);
            assert_eq!(&buf[0..len], &data[0..len]);

            let len = ser.read(&mut buf).expect("len");
            assert_eq!(len, 1);
            assert_eq!(&buf[0..len], &data[128..128 + len]);

            let blk = ser.read(&mut buf).expect_err("block");
        })
        .expect("with_usb");
}

#[test]
fn test_long_read2() {
    MkSerial {}
        .with_usb(|mut ser, mut dev| {
            let data: &[u8] = &(0..256)
                .flat_map(|a| (a as u16).to_le_bytes())
                .collect::<Vec<_>>();
            assert_eq!(data.len(), 512);

            let mut buf = [0u8; 1024];

            let len = dev.ep_write(&mut ser, 1, &data[0..321]).expect("len");
            assert_eq!(len, 321);

            // This returns 32 bytes and (probably) consumes another
            // 64 bytes from the endpoint buffer, as
            // endpoint buffer must be consumed fully.
            //
            // Serial buffer should have 32 and 64 bytes left.
            let len = ser.read(&mut buf[..32]).expect("len");
            assert_eq!(len, 32);
            assert_eq!(&buf[0..len], &data[0..len]);

            // this returns 64 bytes and (probably) consumes another
            // 64 bytes from the endpoint buffer, as
            // endpoint buffer must be consumed fully.
            let len = ser.read(&mut buf).expect("len");
            assert_eq!(len, 96);
            assert_eq!(&buf[0..len], &data[32..32 + len]);

            // So far, 128 out of 321 bytes were read by serial.
            //
            // Write to EP buffer again while Class haven't
            // emptied it.
            let len = dev.ep_write(&mut ser, 1, &data[321..511]).expect("len");
            assert_eq!(len, 190);

            let len = ser.read(&mut buf).expect("len");
            assert_eq!(len, 64);
            assert_eq!(&buf[0..len], &data[128..128 + len]);

            let len = ser.read(&mut buf).expect("len");
            assert_eq!(len, 64);
            assert_eq!(&buf[0..len], &data[192..192 + len]);

            let len = ser.read(&mut buf[..32]).expect("len");
            assert_eq!(len, 32);
            assert_eq!(&buf[0..len], &data[256..256 + len]);

            // With these 32-byte reads it was able to pull
            // another 64 bytes from the endpoint.
            let len = ser.read(&mut buf[..32]).expect("len");
            assert_eq!(len, 32);
            assert_eq!(&buf[0..len], &data[256 + 32..256 + 32 + len]);

            let len = ser.read(&mut buf).expect("len");
            assert_eq!(len, 128);
            assert_eq!(&buf[0..len], &data[320..320 + len]);

            let len = ser.read(&mut buf).expect("len");
            assert_eq!(len, 63);
            assert_eq!(&buf[0..len], &data[448..448 + len]);

            let blk = ser.read(&mut buf).expect_err("block");
        })
        .expect("with_usb");
}

#[test]
fn test_long_write() {
    MkSerial {}
        .with_usb(|mut ser, mut dev| {
            let mut buf = [0u8; 1024];

            buf[0..256]
                .iter_mut()
                .enumerate()
                .map(|(a, b)| *b = a as u8)
                .last();

            // default buffer is 128 bytes
            let len = ser.write(&buf[0..256]).expect("len");
            assert_eq!(len, 128);

            let vec = dev.ep_read(&mut ser, 2, 1024).expect("vec");
            assert_eq!(vec, buf[0..len]);

            let vec = dev.ep_read(&mut ser, 2, 1024).expect("vec");
            assert_eq!(vec, []);
        })
        .expect("with_usb");
}
