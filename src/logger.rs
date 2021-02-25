use crate::hal::hal::serial;
use core::cell::UnsafeCell;
use core::fmt::{self, Write};
use log::{Metadata, Record};
use nb::block;

pub struct Logger<T> {
    inner: UnsafeCell<Inner<T>>,
}

struct Inner<T> {
    stdout: Option<T>,
}

unsafe impl<T> Sync for Logger<T> {}

impl<T> Logger<T> {
    pub const fn new() -> Self {
        Logger {
            inner: UnsafeCell::new(Inner { stdout: None }),
        }
    }

    /// # Safety
    pub unsafe fn set_inner(&self, inner: T) {
        let _ = (*self.inner.get()).stdout.replace(inner);
    }
}

impl<T: Send + serial::Write<u8>> log::Log for Logger<T> {
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

impl<T: serial::Write<u8>> fmt::Write for Inner<T> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if let Some(stdout) = &mut self.stdout {
            for c in s.as_bytes() {
                block!(stdout.write(*c)).ok();
            }
        }
        Ok(())
    }
}
