use crate::{
    colors, Button, InfallibleLedDriver, Instant, IrCommand, SystemClock, White, RGBW, RGBW8,
};
use private::*;

// TODO
// brightness handling
// on/off fading/ramping
// idle state with duration idle for system reset?
// cleanup debug logs

const AUTO_ON_DURATION: Instant = Instant::ONE_MINUTE;
const MANUAL_ON_DURATION: Instant = Instant::ONE_MINUTE;

//const AUTO_ON_DURATION: Instant = Instant::TEN_MINUTES;
//const MANUAL_ON_DURATION: Instant = Instant::ONE_HOUR;

/// Color used for AutoOn and ManualOn
pub const DEFAULT_COLOR: RGBW8 = RGBW {
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

    /// Call this on a timer, 1~10 ms should do
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
                self.sm.process_event(Events::ManualOn(DEFAULT_COLOR)).ok();
            }
            Button::White => {
                let color = RGBW8::new_alpha(0, 0, 0, White(32));
                self.sm.process_event(Events::ManualOn(color)).ok();
            }
            Button::Green => {
                //let color = colors::GREEN.new_alpha(White(0));
                let color = RGBW8::new_alpha(0, 32, 0, White(0));
                self.sm.process_event(Events::ManualOn(color)).ok();
            }
            Button::Red => {
                //let color = colors::RED.new_alpha(White(0));
                let color = RGBW8::new_alpha(32, 0, 0, White(0));
                self.sm.process_event(Events::ManualOn(color)).ok();
            }
            Button::Blue => {
                //let color = colors::BLUE.new_alpha(White(0));
                let color = RGBW8::new_alpha(0, 0, 32, White(0));
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
        InfallibleLedDriver, Instant, SystemClock, AUTO_ON_DURATION, DEFAULT_COLOR,
        MANUAL_ON_DURATION, RGBW8,
    };
    use core::cmp;
    use log::debug;
    use smlang::statemachine;

    #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
    pub enum Mode {
        AutoOn,
        ManualOn,
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
            self.driver.set_pixels(&DEFAULT_COLOR);
            OnStateData {
                mode: Mode::AutoOn,
                color: DEFAULT_COLOR,
                started_at: self.clock.now(),
            }
        }

        fn on_to_off_action(&mut self, _state_data: &OnStateData) {
            debug!("Entered Off");
            /*
            let max_rg = cmp::max(state_data.color.r, state_data.color.g);
            let max_ba = cmp::max(state_data.color.b, state_data.color.a.0);
            let max_channel = cmp::max(max_rg, max_ba);
            let mut color = state_data.color.clone();
            for _ in 0..max_channel {
                color.r = color.r.saturating_sub(1);
                color.g = color.g.saturating_sub(1);
                color.b = color.b.saturating_sub(1);
                color.a.0 = color.a.0.saturating_sub(1);
                self.driver.set_pixels(&color);
                // TODO - DELAY some ms (3), add a Delay impl to SystemClock
                // or tweak the watchdog
                // or some fade_on / fade_off methods
                // so that other cmds can interrupt it
                cortex_m::asm::delay(500_000);
            }
            */
            self.driver.set_off();
        }

        fn on_timer_check_guard(&mut self, state_data: &OnStateData) -> bool {
            if state_data.mode == Mode::AutoOn {
                self.clock.now().saturation_sub(state_data.started_at) >= AUTO_ON_DURATION
            } else {
                self.clock.now().saturation_sub(state_data.started_at) >= MANUAL_ON_DURATION
            }
        }

        fn manual_on_action(&mut self, event_data: &RGBW8) -> OnStateData {
            debug!("Entered ManualOn {:?}", event_data);
            self.driver.set_pixels(event_data);
            OnStateData {
                mode: Mode::ManualOn,
                color: *event_data,
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
        pub color: RGBW8,
        pub started_at: Instant,
    }
}
