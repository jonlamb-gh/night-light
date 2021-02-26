use core::{cmp::Ordering, fmt, iter};
use embedded_hal::spi::FullDuplex;
use log::error;
use smart_leds::SmartLedsWrite;
use ws2812_spi::{devices::Sk6812w, Ws2812};

pub use smart_leds::{colors, hsv::White, RGBW};
#[allow(clippy::upper_case_acronyms)]
pub type RGBW8 = RGBW<u8>;

// TODO - BasicColor enum with all variants on the remote
// iterator/enumerator for the fade/strobe modes to walk
pub const COLOR_OFF: RGBW8 = RGBW {
    r: 0,
    g: 0,
    b: 0,
    a: White(0),
};

pub trait FadeOffRgbw {
    fn set_off(&mut self);
    fn is_off(&self) -> bool;
    fn step_down(&mut self);
}

// TODO - this can just be a wrapper to FadeToRgbw
impl FadeOffRgbw for RGBW8 {
    fn set_off(&mut self) {
        self.r = 0;
        self.g = 0;
        self.b = 0;
        self.a.0 = 0;
    }

    fn is_off(&self) -> bool {
        self.r == 0 && self.g == 0 && self.b == 0 && self.a.0 == 0
    }

    fn step_down(&mut self) {
        self.r = self.r.saturating_sub(1);
        self.g = self.g.saturating_sub(1);
        self.b = self.b.saturating_sub(1);
        self.a.0 = self.a.0.saturating_sub(1);
    }
}

pub trait FadeToRgbw {
    fn destination_reached(&self, destination: &RGBW8) -> bool;

    fn step_to(&mut self, destination: &RGBW8);
}

impl FadeToRgbw for RGBW8 {
    fn destination_reached(&self, destination: &RGBW8) -> bool {
        self == destination
    }

    fn step_to(&mut self, destination: &RGBW8) {
        match self.r.cmp(&destination.r) {
            Ordering::Greater => self.r = self.r.saturating_sub(1),
            Ordering::Less => self.r = self.r.saturating_add(1),
            Ordering::Equal => (),
        }
        match self.g.cmp(&destination.g) {
            Ordering::Greater => self.g = self.g.saturating_sub(1),
            Ordering::Less => self.g = self.g.saturating_add(1),
            Ordering::Equal => (),
        }
        match self.b.cmp(&destination.b) {
            Ordering::Greater => self.b = self.b.saturating_sub(1),
            Ordering::Less => self.b = self.b.saturating_add(1),
            Ordering::Equal => (),
        }
        match self.a.0.cmp(&destination.a.0) {
            Ordering::Greater => self.a.0 = self.a.0.saturating_sub(1),
            Ordering::Less => self.a.0 = self.a.0.saturating_add(1),
            Ordering::Equal => (),
        }
    }
}

// TODO - add brightness later
pub trait InfallibleLedDriver {
    const NUM_LEDS: usize;

    fn set_pixels(&mut self, color: &RGBW8);

    fn set_off(&mut self) {
        self.set_pixels(&COLOR_OFF);
    }
}

pub struct InfallibleSk6812w<SPI>(Ws2812<SPI, Sk6812w>);

impl<SPI> From<Ws2812<SPI, Sk6812w>> for InfallibleSk6812w<SPI> {
    fn from(t: Ws2812<SPI, Sk6812w>) -> Self {
        InfallibleSk6812w(t)
    }
}

impl<SPI, E> InfallibleLedDriver for InfallibleSk6812w<SPI>
where
    SPI: FullDuplex<u8, Error = E>,
    E: fmt::Debug,
{
    // TODO testing on the 8 pixel strip, the ring has 12
    //const NUM_LEDS: usize = 8;
    const NUM_LEDS: usize = 1;

    fn set_pixels(&mut self, color: &RGBW8) {
        let pixels = iter::repeat(color).take(Self::NUM_LEDS);

        // Unwrap/panic ok, will trigger watchdog reset
        self.0
            .write(pixels.cloned())
            .map_err(|e| error!("Failed to set pixels {:?}", e))
            .unwrap();
    }
}

/*
struct Brightness<I> {
    iter: I,
    brightness: u8,
}

impl<'a, I> Iterator for Brightness<I>
where
    I: Iterator<Item = RGBW<u8>>,
{
    type Item = RGBW<u8>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|p| RGBW {
            r: (p.r as u16 * (self.brightness as u16 + 1) / 256) as u8,
            g: (p.g as u16 * (self.brightness as u16 + 1) / 256) as u8,
            b: (p.b as u16 * (self.brightness as u16 + 1) / 256) as u8,
            a: White((p.a.0 as u16 * (self.brightness as u16 + 1) / 256) as u8),
        })
    }
}

fn brightness_iter<I>(iter: I, brightness: u8) -> Brightness<I>
where
    I: Iterator<Item = RGBW<u8>>,
{
    Brightness { iter, brightness }
}
*/
