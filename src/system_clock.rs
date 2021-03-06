use core::sync::atomic::{AtomicU32, Ordering::SeqCst};
use core::{fmt, ops};
use hal::rcc::Clocks;
use hal::stm32::SYST;
use log::debug;

/// Milliseconds
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
#[repr(transparent)]
pub struct Instant(u32);

pub type Duration = Instant;

impl Instant {
    pub const ZERO: Self = Instant(0);
    pub const ONE_SECOND: Self = Instant(1000);
    pub const ONE_MINUTE: Self = Instant(1000 * 60);
    pub const TEN_MINUTES: Self = Instant(1000 * 60 * 10);
    pub const ONE_HOUR: Self = Instant(1000 * 60 * 60);
    pub const TEN_HOURS: Self = Instant(1000 * 60 * 60 * 10);

    pub const fn from_millis(ms: u32) -> Self {
        Instant(ms)
    }

    pub fn as_millis(self) -> u32 {
        self.0
    }

    pub fn duration_since(&self, earlier: Instant) -> Duration {
        Duration::from_millis(
            self.0
                .checked_sub(earlier.0)
                .expect("Supplied instant is later than self"),
        )
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

impl ops::Add for Instant {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Instant(self.0 + other.0)
    }
}

/// 32-bit millisecond clock
#[derive(Debug)]
pub struct SystemClock(AtomicU32);

unsafe impl Send for SystemClock {}
unsafe impl Sync for SystemClock {}

impl SystemClock {
    pub const NEAR_WRAP_AROUND_VALUE: Instant = Instant(core::u32::MAX - Instant::TEN_HOURS.0);

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

    pub fn is_near_wrap_around(&self) -> bool {
        self.now().as_millis() >= Self::NEAR_WRAP_AROUND_VALUE.as_millis()
    }

    pub fn now(&self) -> Instant {
        Instant::from_millis(self.load())
    }

    pub fn duration_since(&self, earlier: Instant) -> Duration {
        self.now().duration_since(earlier)
    }

    fn load(&self) -> u32 {
        self.0.load(SeqCst)
    }
}
