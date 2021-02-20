use crate::led_control::{colors, White, RGBW8};
use crate::{Button, Instant, IrCommand, LedController, SystemClock};
use embedded_hal::spi::FullDuplex;
use log::debug;
use private::*;

// TODO
// default brightness instead max
// default on color, same as auto-on
// on/off fading/ramping, probably a FadingOn/Off state
//   maybe a fade_of method in the led controller, takes a Delay impl ref and
//   duration, or do the timing in the state machine
//
// idle state with duration idle for system reset

const AUTO_ON_DURATION: Instant = Instant::TEN_MINUTES;
const MANUAL_ON_DURATION: Instant = Instant::ONE_MINUTE;
//const MANUAL_ON_DURATION: Instant = Instant::ONE_HOUR;

pub struct Controller<SPI: FullDuplex<u8>> {
    sm: StateMachine<Context<SPI>>,
}

impl<SPI, E> Controller<SPI>
where
    SPI: FullDuplex<u8, Error = E>,
{
    pub fn new(led_controller: LedController<SPI>, sys_clock: &'static SystemClock) -> Self {
        let mut sm = StateMachine::new(Context::new(led_controller, sys_clock));
        sm.process_event(Events::Init).ok();
        Controller { sm }
    }

    pub fn handle_auto_on_event(&mut self) {
        self.sm.process_event(Events::AutoOn).ok();
    }

    pub fn handle_ir_command(&mut self, cmd: IrCommand) {
        debug!("Handling {}", cmd);
        match cmd.button {
            Button::BrightnessDown => self.sm.context_mut().decrease_brightness(),
            Button::BrightnessUp => self.sm.context_mut().increase_brightness(),
            Button::Off => {
                self.sm.process_event(Events::Off).ok();
            }
            Button::On => {
                // TODO - default on color
                let color = RGBW8::new_alpha(0, 0, 0, White(255));
                self.sm.process_event(Events::ManualColor(color)).ok();
            }
            Button::Flash => {
                self.sm
                    .process_event(Events::ManualMode(ManualMode::Flash))
                    .ok();
            }
            Button::Smooth => {
                self.sm
                    .process_event(Events::ManualMode(ManualMode::Smooth))
                    .ok();
            }
            Button::Strobe => {
                self.sm
                    .process_event(Events::ManualMode(ManualMode::Strobe))
                    .ok();
            }
            Button::Fade => {
                self.sm
                    .process_event(Events::ManualMode(ManualMode::Fade))
                    .ok();
            }
            // TODO - check colors
            // RGBA { r: 0, g: 128, b: 0, a: White(0) }
            Button::Green => {
                let color = colors::GREEN.new_alpha(White(0));
                self.sm.process_event(Events::ManualColor(color)).ok();
            }
            _ => (), // TODO
        }
    }

    // TODO - call this on a timer, 1ms or so, depending on how ManualMode is done
    pub fn update(&mut self) {
        self.sm.process_event(Events::TimerCheck).ok();
    }
}

mod private {
    use super::{AUTO_ON_DURATION, MANUAL_ON_DURATION};
    use crate::led_control::{White, RGBW8};
    use crate::{Instant, LedController, SystemClock};
    use embedded_hal::spi::FullDuplex;
    use log::{debug, error};
    use smlang::statemachine;

    #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
    pub enum ManualMode {
        Flash,
        Smooth,
        Strobe,
        Fade,
    }

    statemachine! {
        *Reset + Init / init_action = Off,

        Off + AutoOn / auto_on_action = AutoOn,
        AutoOn(AutoOnStateData) + TimerCheck [auto_on_timer_check_guard] / auto_on_to_off_action = Off,
        AutoOn(AutoOnStateData) + Off / auto_on_to_off_action = Off,

        Off + ManualColor(RGBW8) / manual_color_action = ManualColor,
        ManualColor(ManualColorStateData) + TimerCheck [manual_color_timer_check_guard] / mc_to_off_action = Off,
        ManualColor(ManualColorStateData) + Off / mc_to_off_action = Off,

        Off + ManualMode(ManualMode) / manual_mode_action = ManualMode,
        ManualMode(ManualModeStateData) + TimerCheck [manual_mode_timer_check_guard] / mm_to_off_action = Off,
        ManualMode(ManualModeStateData) + Off / mm_to_off_action = Off,
    }

    pub struct Context<SPI> {
        leds: LedController<SPI>,
        clock: &'static SystemClock,
    }

    impl<SPI, E> Context<SPI>
    where
        SPI: FullDuplex<u8, Error = E>,
    {
        pub fn new(leds: LedController<SPI>, clock: &'static SystemClock) -> Self {
            Context { clock, leds }
        }

        pub fn increase_brightness(&mut self) {
            self.leds.increase_brightness();
        }

        pub fn decrease_brightness(&mut self) {
            self.leds.decrease_brightness();
        }

        fn off(&mut self) {
            debug!("Setting LEDs off");
            self.leds.set_all_off();
            self.update_leds();
        }

        fn update_leds(&mut self) {
            if let Err(e) = self.leds.update_leds() {
                error!("Failed to update LEDS: {}", e);
            }
        }
    }

    impl<SPI, E> StateMachineContext for Context<SPI>
    where
        SPI: FullDuplex<u8, Error = E>,
    {
        fn init_action(&mut self) {
            debug!("Initialized state machine");
            self.leds.set_max_brightness();
            self.leds.set_all_off();
            self.update_leds();
        }

        fn auto_on_action(&mut self) -> AutoOnStateData {
            debug!("Entered AutoOn");
            // TODO - default on color
            self.leds.set_all(RGBW8::new_alpha(0, 0, 0, White(255)));
            self.update_leds();
            AutoOnStateData {
                started_at: self.clock.now(),
            }
        }

        fn auto_on_timer_check_guard(&mut self, state_data: &AutoOnStateData) -> bool {
            self.clock.now().saturation_sub(state_data.started_at) >= AUTO_ON_DURATION
        }

        fn auto_on_to_off_action(&mut self, _state_data: &AutoOnStateData) {
            self.off();
        }

        fn manual_color_action(&mut self, event_data: &RGBW8) -> ManualColorStateData {
            debug!("Entered ManualColor {:?}", event_data);
            self.leds.set_all(*event_data);
            self.update_leds();
            ManualColorStateData {
                started_at: self.clock.now(),
            }
        }

        fn manual_color_timer_check_guard(&mut self, state_data: &ManualColorStateData) -> bool {
            self.clock.now().saturation_sub(state_data.started_at) >= MANUAL_ON_DURATION
        }

        fn mc_to_off_action(&mut self, _state_data: &ManualColorStateData) {
            self.off();
        }

        fn manual_mode_action(&mut self, event_data: &ManualMode) -> ManualModeStateData {
            debug!("Entered ManualMode {:?}", event_data);
            // TODO - mode + color logic, just on for now
            self.leds.set_all(RGBW8::new_alpha(0, 0, 0, White(255)));
            self.update_leds();
            ManualModeStateData {
                mode: *event_data,
                started_at: self.clock.now(),
            }
        }

        fn manual_mode_timer_check_guard(&mut self, state_data: &ManualModeStateData) -> bool {
            // TODO - do mode update logic here
            self.clock.now().saturation_sub(state_data.started_at) >= MANUAL_ON_DURATION
        }

        fn mm_to_off_action(&mut self, _state_data: &ManualModeStateData) {
            self.off();
        }
    }

    #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
    pub struct AutoOnStateData {
        pub started_at: Instant,
    }

    #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
    pub struct ManualColorStateData {
        pub started_at: Instant,
    }

    #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
    pub struct ManualModeStateData {
        pub mode: ManualMode,
        pub started_at: Instant,
    }
}
