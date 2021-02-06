//! Implementation of `Log` over `usbd_serial::SerialPort`.
//! Single core/thread only, don't use it from an interrupt handler either.

use core::borrow::BorrowMut;
use core::cell::UnsafeCell;
use core::fmt::{self, Write};
use embedded_hal::serial;
use log::{Log, Metadata, Record};
use nb::block;
use usb_device::bus::UsbBus;
use usb_device::prelude::*;
use usbd_serial::{SerialPort, UsbError};

pub const DEFAULT_USB_RX_BUFFER_CAPACITY: usize = 64;
pub const DEFAULT_USB_TX_BUFFER_CAPACITY: usize = 128;

pub struct Logger<T> {
    inner: UnsafeCell<Innards<T>>,
}

struct Innards<T> {
    stdout: Option<T>,
}

unsafe impl<T> Sync for Logger<T> {}

impl<T> Logger<T> {
    pub const fn new() -> Self {
        Logger {
            inner: UnsafeCell::new(Innards { stdout: None }),
        }
    }

    pub unsafe fn set_inner(&self, inner: T) {
        let _ = (*self.inner.get()).stdout.replace(inner);
    }

    pub fn inner_mut(&self) -> &mut Option<T> {
        &mut unsafe { &mut *self.inner.get() }.stdout
    }
}

pub struct SerialPortLogger<'a, B: UsbBus, RS: BorrowMut<[u8]>, WS: BorrowMut<[u8]>> {
    port: SerialPort<'a, B, RS, WS>,
}

impl<'a, B, RS, WS> From<SerialPort<'a, B, RS, WS>> for SerialPortLogger<'a, B, RS, WS>
where
    B: UsbBus,
    RS: BorrowMut<[u8]>,
    WS: BorrowMut<[u8]>,
{
    fn from(port: SerialPort<'a, B, RS, WS>) -> Self {
        SerialPortLogger { port }
    }
}

impl<'a, B, RS, WS> SerialPortLogger<'a, B, RS, WS>
where
    B: UsbBus,
    RS: BorrowMut<[u8]>,
    WS: BorrowMut<[u8]>,
{
    pub fn poll(&mut self, dev: &mut UsbDevice<'a, B>) {
        let _rx_or_tx_ready = dev.poll(&mut [&mut self.port]);
    }
}

impl<B, RS, WS> serial::Write<u8> for SerialPortLogger<'_, B, RS, WS>
where
    B: UsbBus,
    RS: BorrowMut<[u8]>,
    WS: BorrowMut<[u8]>,
{
    type Error = UsbError;

    fn write(&mut self, word: u8) -> nb::Result<(), Self::Error> {
        serial::Write::write(&mut self.port, word)
    }

    fn flush(&mut self) -> nb::Result<(), Self::Error> {
        serial::Write::flush(&mut self.port)
    }
}

impl<T: serial::Write<u8>> fmt::Write for Innards<T> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if let Some(stdout) = &mut self.stdout {
            for c in s.as_bytes() {
                block!(stdout.write(*c)).ok();
            }
        }
        Ok(())
    }
}

impl<T: Send + serial::Write<u8>> Log for Logger<T> {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let inner = unsafe { &mut *self.inner.get() };
            writeln!(inner, "[{}] {}", record.level(), record.args()).ok();
        }
    }

    fn flush(&self) {
        let inner = unsafe { &mut *self.inner.get() };
        if let Some(stdout) = &mut inner.stdout {
            block!(stdout.flush()).ok();
        }
    }
}
