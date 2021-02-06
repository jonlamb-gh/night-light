// TODO - use https://crates.io/crates/embedded-time

use core::fmt;
use core::sync::atomic::{AtomicU32, Ordering::SeqCst};
use hal::rcc::Clocks;
use hal::stm32::SYST;
use log::debug;

/// Milliseconds
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
#[repr(transparent)]
pub struct Instant(u32);

impl Instant {
    pub fn from_millis(ms: u32) -> Self {
        Instant(ms)
    }

    pub fn as_millis(self) -> u32 {
        self.0
    }
}

impl From<Instant> for u32 {
    fn from(i: Instant) -> Self {
        i.0
    }
}

impl fmt::Display for Instant {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// 32-bit millisecond clock
#[derive(Debug)]
pub struct SystemClock(AtomicU32);

unsafe impl Send for SystemClock {}
unsafe impl Sync for SystemClock {}

impl SystemClock {
    pub const fn new() -> Self {
        SystemClock(AtomicU32::new(0))
    }

    pub fn enable_systick_interrupt(&self, mut syst: SYST, clocks: Clocks) {
        debug!("Enable SystemClock hclk freq {} Hz", clocks.hclk().0);

        // Generate an interrupt once a millisecond, HCLK/8/1000
        syst.set_reload((clocks.hclk().0 / 8) / 1000);
        syst.clear_current();
        syst.enable_counter();
        syst.enable_interrupt();

        // So the SYST can't be stopped or reset
        drop(syst);
    }

    pub fn inc_from_interrupt(&self) {
        self.0.fetch_add(1, SeqCst);
    }

    pub fn now(&self) -> Instant {
        Instant::from_millis(self.0.load(SeqCst))
    }
}
