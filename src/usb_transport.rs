use core::borrow::BorrowMut;
use usb_device::bus::UsbBus;
use usb_device::prelude::*;
use usbd_serial::{SerialPort, UsbError};

pub const DEFAULT_RX_BUFFER_CAPACITY: usize = 128;
pub const DEFAULT_TX_BUFFER_CAPACITY: usize = 128;

// Mostly a wrapper over
// https://docs.rs/usbd-serial/0.1.1/usbd_serial/struct.SerialPort.html
// and
// https://docs.rs/usb-device/0.2.7/usb_device/device/struct.UsbDevice.html
pub struct UsbTransport<'a, B: UsbBus, RS: BorrowMut<[u8]>, WS: BorrowMut<[u8]>> {
    dev: UsbDevice<'a, B>,
    port: SerialPort<'a, B, RS, WS>,
}

impl<'a, B: UsbBus, RS: BorrowMut<[u8]>, WS: BorrowMut<[u8]>> UsbTransport<'a, B, RS, WS> {
    pub fn new(dev: UsbDevice<'a, B>, port: SerialPort<'a, B, RS, WS>) -> Self {
        UsbTransport { dev, port }
    }

    pub fn state(&self) -> UsbDeviceState {
        self.dev.state()
    }

    // Must be called at least once every 10 milliseconds while connected to the USB
    // host to be USB compliant.
    pub fn poll(&mut self) {
        let _rx_or_tx_ready = self.dev.poll(&mut [&mut self.port]);

        // TODO - store prev_state and check here for transition, debug log it
    }

    pub fn flush(&mut self) -> Result<(), UsbError> {
        self.port.flush()
    }

    // TODO - ignore/drop the write if state != Configured
    pub fn write(&mut self, data: &[u8]) -> Result<(), UsbError> {
        match self.port.write(data) {
            Ok(bytes_written) => {
                if bytes_written != data.len() {
                    Err(UsbError::BufferOverflow)
                } else {
                    Ok(())
                }
            }
            Err(e) => Err(e),
        }
    }
}
