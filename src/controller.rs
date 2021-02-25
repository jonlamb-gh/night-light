use crate::{
    colors, Button, InfallibleLedDriver, Instant, IrCommand, SystemClock, White, RGBW, RGBW8,
};
use private::*;

// TODO
// default brightness instead max
// default on color, same as auto-on
// on/off fading/ramping, probably a FadingOn/Off state
//   maybe a fade_of method in the led controller, takes a Delay impl ref and
//   duration, or do the timing in the state machine
//
// idle state with duration idle for system reset?
//
// cleanup debug logs

const AUTO_ON_DURATION: Instant = Instant::ONE_MINUTE;
const MANUAL_ON_DURATION: Instant = Instant::ONE_MINUTE;

//const AUTO_ON_DURATION: Instant = Instant::TEN_MINUTES;
//const MANUAL_ON_DURATION: Instant = Instant::ONE_HOUR;

pub const COLOR_ON: RGBW8 = RGBW {
    r: 10,
    g: 0,
    b: 0,
    a: White(10),
};

pub struct Controller<LED: InfallibleLedDriver> {
    sm: StateMachine<Context<LED>>,
}

impl<LED> Controller<LED>
where
    LED: InfallibleLedDriver,
{
    pub fn new(driver: LED, sys_clock: &'static SystemClock) -> Self {
        let mut sm = StateMachine::new(Context::new(driver, sys_clock));
        sm.process_event(Events::Init).ok();
        Controller { sm }
    }

    // TODO - call this on a timer, 1ms or so, depending on how ManualMode is done
    pub fn update(&mut self) {
        self.sm.process_event(Events::TimerCheck).ok();
    }

    pub fn handle_auto_on_event(&mut self) {
        self.sm.process_event(Events::AutoOn).ok();
    }

    pub fn handle_ir_command(&mut self, cmd: IrCommand) {
        match cmd.button {
            Button::Off => {
                self.sm.process_event(Events::Off).ok();
            }
            Button::On => {
                self.sm.process_event(Events::ManualOn(COLOR_ON)).ok();
            }
            Button::Green => {
                let color = colors::GREEN.new_alpha(White(0));
                self.sm.process_event(Events::ManualOn(color)).ok();
            }
            Button::Red => {
                let color = colors::RED.new_alpha(White(0));
                self.sm.process_event(Events::ManualOn(color)).ok();
            }
            Button::Blue => {
                let color = colors::BLUE.new_alpha(White(0));
                self.sm.process_event(Events::ManualOn(color)).ok();
            }
            /*
            Button::Flash => {
                self.sm
                    .process_event(Events::ManualMode(ManualMode::Flash))
                    .ok();
            }
            */
            _ => (), // TODO
        }
    }
}

mod private {
    use super::{
        InfallibleLedDriver, Instant, SystemClock, AUTO_ON_DURATION, COLOR_ON, MANUAL_ON_DURATION,
        RGBW8,
    };
    use log::debug;
    use smlang::statemachine;

    #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
    pub enum Mode {
        Constant,
        /*Flash,
         *Smooth,
         *Strobe,
         *Fade, */
    }

    statemachine! {
        *Reset + Init / init_action = Off,

        Off + AutoOn / auto_on_action = On,
        Off + ManualOn(RGBW8) / manual_on_action = On,

        On(OnStateData) + Off / on_to_off_action = Off,
        On(OnStateData) + TimerCheck [on_timer_check_guard] / on_to_off_action = Off,

        On(OnStateData) + ManualOn(RGBW8) / manual_on_to_on_action = On,
        On(OnStateData) + AutoOn / auto_on_to_on_action = On,
    }

    /*
        Off + AutoOn / auto_on_action = AutoOn,
        AutoOn(AutoOnStateData) + TimerCheck [auto_on_timer_check_guard] / auto_on_to_off_action = Off,
        AutoOn(AutoOnStateData) + Off / auto_on_to_off_action = Off,

        Off + ManualColor(RGBW8) / manual_color_action = ManualColor,
        ManualColor(ManualColorStateData) + TimerCheck [manual_color_timer_check_guard] / mc_to_off_action = Off,
        ManualColor(ManualColorStateData) + Off / mc_to_off_action = Off,

        Off + ManualMode(ManualMode) / manual_mode_action = ManualMode,
        ManualMode(ManualModeStateData) + TimerCheck [manual_mode_timer_check_guard] / mm_to_off_action = Off,
        ManualMode(ManualModeStateData) + Off / mm_to_off_action = Off,
    */

    pub struct Context<LED> {
        driver: LED,
        clock: &'static SystemClock,
        // TODO - store current color for fade on/off
    }

    impl<LED> Context<LED>
    where
        LED: InfallibleLedDriver,
    {
        pub fn new(driver: LED, clock: &'static SystemClock) -> Self {
            Context { driver, clock }
        }
    }

    impl<LED> StateMachineContext for Context<LED>
    where
        LED: InfallibleLedDriver,
    {
        fn init_action(&mut self) {
            debug!("Initialized LED controller state machine");
            self.driver.set_off();
        }

        fn auto_on_action(&mut self) -> OnStateData {
            debug!("Entered AutoOn");
            self.driver.set_pixels(&COLOR_ON);
            OnStateData {
                mode: Mode::Constant,
                started_at: self.clock.now(),
            }
        }

        fn on_to_off_action(&mut self, _state_data: &OnStateData) {
            debug!("Entered Off");
            self.driver.set_off();
        }

        fn on_timer_check_guard(&mut self, state_data: &OnStateData) -> bool {
            // TODO auto vs manual duration
            self.clock.now().saturation_sub(state_data.started_at) >= AUTO_ON_DURATION
        }

        fn manual_on_action(&mut self, event_data: &RGBW8) -> OnStateData {
            debug!("Entered ManualOn {:?}", event_data);
            self.driver.set_pixels(event_data);
            OnStateData {
                mode: Mode::Constant,
                started_at: self.clock.now(),
            }
        }

        fn manual_on_to_on_action(
            &mut self,
            _state_data: &OnStateData,
            event_data: &RGBW8,
        ) -> OnStateData {
            self.manual_on_action(event_data)
        }

        fn auto_on_to_on_action(&mut self, _state_data: &OnStateData) -> OnStateData {
            self.auto_on_action()
        }
    }

    #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
    pub struct OnStateData {
        pub mode: Mode,
        pub started_at: Instant,
    }
}
