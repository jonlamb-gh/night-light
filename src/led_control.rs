use core::fmt;
use embedded_hal::spi::FullDuplex;
use smart_leds::{hsv::White, SmartLedsWrite, RGB8, RGBW};
use ws2812_spi::{devices::Sk6812w, Ws2812};

pub use smart_leds::colors;

pub const NUM_LEDS: usize = 12;

pub struct LedController<SPI> {
    driver: Ws2812<SPI, Sk6812w>,
    brightness: u8,
    pixels: [RGBW<u8>; NUM_LEDS],
}

impl<SPI, E> LedController<SPI>
where
    SPI: FullDuplex<u8, Error = E>,
    E: fmt::Debug,
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

    pub fn set_all(&mut self, color: RGB8, white: u8) -> Result<(), E> {
        for p in self.pixels.iter_mut() {
            p.r = color.r;
            p.g = color.g;
            p.b = color.b;
            p.a = White(white);
        }

        self.write_leds()
    }

    fn write_leds(&mut self) -> Result<(), E> {
        let pixels_iter = self.pixels.iter().cloned();
        if self.brightness != core::u8::MAX {
            self.driver
                .write(brightness_iter(pixels_iter, self.brightness))
        } else {
            self.driver.write(pixels_iter)
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
