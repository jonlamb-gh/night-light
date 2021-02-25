use core::{fmt, iter};
use embedded_hal::spi::FullDuplex;
use log::error;
use smart_leds::SmartLedsWrite;
use ws2812_spi::{devices::Sk6812w, Ws2812};

pub use smart_leds::{colors, hsv::White, RGBW};
#[allow(clippy::upper_case_acronyms)]
pub type RGBW8 = RGBW<u8>;

pub const COLOR_OFF: RGBW8 = RGBW {
    r: 0,
    g: 0,
    b: 0,
    a: White(0),
};

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
    const NUM_LEDS: usize = 8;

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
