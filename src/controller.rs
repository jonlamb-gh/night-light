use crate::led_control::{colors, White, RGBW};
use crate::{Button, IrCommand, LedController};
use embedded_hal::{spi::FullDuplex, timer::CountDown};
use err_derive::Error;
use hal::time::Hertz;

// TODO
// default brightness
// default on color, same as auto-on
// errors
// state machine for auto-on/off stuff

// TODO - may need to use the system clock instead for period > 1s
// https://github.com/stm32-rs/stm32f3xx-hal/issues/190
pub const TIMER_FREQ: Hertz = Hertz(10);

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Error)]
pub enum Error {
    #[error(display = "LED controller error {}", _0)]
    LedController(#[error(source)] crate::led_control::Error),
}

pub struct Controller<SPI, TIMER> {
    leds: LedController<SPI>,
    timer: TIMER,
}

impl<SPI, TIMER, E> Controller<SPI, TIMER>
where
    SPI: FullDuplex<u8, Error = E>,
    TIMER: CountDown,
{
    pub fn new(led_controller: LedController<SPI>, timer: TIMER) -> Self {
        Controller {
            leds: led_controller,
            timer,
        }
    }

    pub fn initialize(&mut self) -> Result<(), Error> {
        self.leds.set_max_brightness();
        self.leds.set_all_off();
        self.leds.update_leds()?;
        Ok(())
    }

    pub fn handle_auto_on_event(&mut self) -> Result<(), Error> {
        self.leds.set_all(RGBW::new_alpha(0, 0, 0, White(255)));
        self.leds.update_leds()?;
        // TODO - timer
        Ok(())
    }

    pub fn handle_ir_command(&mut self, cmd: IrCommand) -> Result<(), Error> {
        match cmd.button {
            Button::BrightnessDown => self.leds.decrease_brightness(),
            Button::BrightnessUp => self.leds.increase_brightness(),
            Button::Off => self.leds.set_all_off(),
            _ => return Ok(()),
        }
        self.leds.update_leds()?;
        Ok(())
    }
}
