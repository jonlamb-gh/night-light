use crate::Button;
use core::{cmp::Ordering, fmt, iter};
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

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum BasicColor {
    Red,
    Tomato,
    DarkOrange,
    Orange,
    Yellow,

    Green,
    GreenYellow,
    DarkOliveGreen,
    DarkSeaGreen,
    LightGreen,

    Blue,
    SkyBlue,
    Violet,
    PaleVioletRed,
    Magenta,
}

impl BasicColor {
    pub fn enumerate() -> &'static [Self] {
        use BasicColor::*;
        &[
            Red,
            Tomato,
            DarkOrange,
            Orange,
            Yellow,
            Green,
            GreenYellow,
            DarkOliveGreen,
            DarkSeaGreen,
            LightGreen,
            Blue,
            SkyBlue,
            Violet,
            PaleVioletRed,
            Magenta,
        ]
    }

    pub fn as_rgbw(self) -> RGBW8 {
        use BasicColor::*;
        match self {
            Red => colors::RED.new_alpha(White(0)),
            Tomato => colors::TOMATO.new_alpha(White(0)),
            DarkOrange => colors::DARK_ORANGE.new_alpha(White(0)),
            Orange => colors::ORANGE.new_alpha(White(0)),
            Yellow => colors::YELLOW.new_alpha(White(0)),

            Green => colors::GREEN.new_alpha(White(0)),
            GreenYellow => colors::GREEN_YELLOW.new_alpha(White(0)),
            DarkOliveGreen => colors::DARK_OLIVE_GREEN.new_alpha(White(0)),
            DarkSeaGreen => colors::DARK_SEA_GREEN.new_alpha(White(0)),
            LightGreen => colors::LIGHT_GREEN.new_alpha(White(0)),

            Blue => colors::BLUE.new_alpha(White(0)),
            SkyBlue => colors::SKY_BLUE.new_alpha(White(0)),
            Violet => colors::VIOLET.new_alpha(White(0)),
            PaleVioletRed => colors::PALE_VIOLET_RED.new_alpha(White(0)),
            Magenta => colors::MAGENTA.new_alpha(White(0)),
        }
    }

    pub fn from_button(b: Button) -> Option<Self> {
        use BasicColor::*;
        Some(match b {
            Button::Red => Red,
            Button::Red1 => Tomato,
            Button::Red2 => DarkOrange,
            Button::Red3 => Orange,
            Button::Red4 => Yellow,
            Button::Green => Green,
            Button::Green1 => GreenYellow,
            Button::Green2 => DarkOliveGreen,
            Button::Green3 => DarkSeaGreen,
            Button::Green4 => LightGreen,
            Button::Blue => Blue,
            Button::Blue1 => SkyBlue,
            Button::Blue2 => Violet,
            Button::Blue3 => PaleVioletRed,
            Button::Blue4 => Magenta,
            _ => return None,
        })
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct RandomColorGen(oorandom::Rand32);

impl RandomColorGen {
    pub fn new(seed: u64) -> Self {
        RandomColorGen(oorandom::Rand32::new(seed))
    }

    pub fn rand_rgb(&mut self) -> RGBW8 {
        RGBW8::new_alpha(
            self.0.rand_range(0..256) as u8,
            self.0.rand_range(0..256) as u8,
            self.0.rand_range(0..256) as u8,
            White(0),
        )
    }
}

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
