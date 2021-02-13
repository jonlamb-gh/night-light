use embedded_hal::spi::FullDuplex;
use err_derive::Error;
use smart_leds::SmartLedsWrite;
use ws2812_spi::{devices::Sk6812w, Ws2812};

pub use smart_leds::{colors, hsv::White, RGBW};

pub const NUM_LEDS: usize = 12;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Error)]
pub enum Error {
    #[error(display = "Hw SPI error")]
    HwSpi,
}

pub struct LedController<SPI> {
    driver: Ws2812<SPI, Sk6812w>,
    brightness: u8,
    pixels: [RGBW<u8>; NUM_LEDS],
}

impl<SPI, E> LedController<SPI>
where
    SPI: FullDuplex<u8, Error = E>,
{
    pub fn new(driver: Ws2812<SPI, Sk6812w>) -> Self {
        LedController {
            driver,
            brightness: core::u8::MAX,
            pixels: [RGBW::default(); NUM_LEDS],
        }
    }

    pub fn set_max_brightness(&mut self) {
        self.brightness = core::u8::MAX;
    }

    pub fn increase_brightness(&mut self) {
        self.brightness = self.brightness.saturating_add(1);
    }

    pub fn decrease_brightness(&mut self) {
        self.brightness = self.brightness.saturating_sub(1);
    }

    pub fn set_all(&mut self, color: RGBW<u8>) {
        for p in self.pixels.iter_mut() {
            p.r = color.r;
            p.g = color.g;
            p.b = color.b;
            p.a = color.a;
        }
    }

    pub fn set_all_off(&mut self) {
        self.set_all(RGBW::new_alpha(0, 0, 0, White(0)));
    }

    pub fn update_leds(&mut self) -> Result<(), Error> {
        let pixels_iter = self.pixels.iter().cloned();
        if self.brightness != core::u8::MAX {
            self.driver
                .write(brightness_iter(pixels_iter, self.brightness))
                .map_err(|_| Error::HwSpi)
        } else {
            self.driver.write(pixels_iter).map_err(|_| Error::HwSpi)
        }
    }
}

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
