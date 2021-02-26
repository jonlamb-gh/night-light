use crate::{
    colors, Button, InfallibleLedDriver, Instant, IrCommand, SystemClock, White, RGBW, RGBW8,
};
use private::{Context, Events, StateMachine};

// TODO
// brightness handling
// on/off fading/ramping
// idle state with duration idle for system reset?
// cleanup debug logs

const AUTO_ON_DURATION: Instant = Instant::ONE_MINUTE;
const MANUAL_ON_DURATION: Instant = Instant::ONE_MINUTE;

const FADE_OFF_STEP_DURATION: Instant = Instant::from_millis(10);

//const AUTO_ON_DURATION: Instant = Instant::TEN_MINUTES;
//const MANUAL_ON_DURATION: Instant = Instant::ONE_HOUR;

/// Color used for AutoOn and ManualOn
const DEFAULT_COLOR: RGBW8 = RGBW {
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
                if !cmd.repeat {
                    self.sm.process_event(Events::Off).ok();
                }
            }
            Button::On => {
                self.sm.process_event(Events::ManualOn(DEFAULT_COLOR)).ok();
            }
            Button::White => {
                let color = RGBW8::new_alpha(0, 0, 0, White(255));
                self.sm.process_event(Events::ManualOn(color)).ok();
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
    use super::{AUTO_ON_DURATION, DEFAULT_COLOR, FADE_OFF_STEP_DURATION, MANUAL_ON_DURATION};
    use crate::{FadeOffRgbw, InfallibleLedDriver, Instant, SystemClock, RGBW8};
    use core::cell::RefCell;
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

        FadeOff(RefCell<FadeOffStateData>) + TimerCheck [fade_off_timer_check_guard] / fade_off_to_off_action = Off,
        // Off again while fading will force off
        FadeOff(RefCell<FadeOffStateData>) + Off / fade_off_to_off_action = Off,

        // Manual/Auto-On while in FadeOff ok
        FadeOff(RefCell<FadeOffStateData>) + ManualOn(RGBW8) / fade_off_to_on_action = On,
        FadeOff(RefCell<FadeOffStateData>) + AutoOn / fade_off_auto_on_to_on_action = On,

        On(OnStateData) + Off / on_to_fade_off_action = FadeOff,
        On(OnStateData) + TimerCheck [on_timer_check_guard] / on_to_fade_off_action = FadeOff,

        On(OnStateData) + ManualOn(RGBW8) / manual_on_to_on_action = On,
        On(OnStateData) + AutoOn / auto_on_to_on_action = On,
    }

    pub struct Context<LED> {
        driver: LED,
        clock: &'static SystemClock,
    }

    impl<LED> Context<LED>
    where
        LED: InfallibleLedDriver,
    {
        pub fn new(driver: LED, clock: &'static SystemClock) -> Self {
            Context { driver, clock }
        }

        fn common_enter_off_state(&mut self) {
            debug!("Entered Off");
            self.driver.set_off();
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

        fn fade_off_to_on_action(
            &mut self,
            _state_data: &RefCell<FadeOffStateData>,
            event_data: &RGBW8,
        ) -> OnStateData {
            self.manual_on_action(event_data)
        }

        fn on_to_fade_off_action(&mut self, state_data: &OnStateData) -> RefCell<FadeOffStateData> {
            debug!("Entered FadeOff");
            RefCell::new(FadeOffStateData {
                color: state_data.color,
                next_transition_at: self.clock.now() + FADE_OFF_STEP_DURATION,
            })
        }

        fn fade_off_to_off_action(&mut self, _state_data: &RefCell<FadeOffStateData>) {
            self.common_enter_off_state();
        }

        fn fade_off_auto_on_to_on_action(
            &mut self,
            _state_data: &RefCell<FadeOffStateData>,
        ) -> OnStateData {
            self.auto_on_action()
        }

        fn fade_off_timer_check_guard(&mut self, state_data: &RefCell<FadeOffStateData>) -> bool {
            if !state_data.borrow().color.is_off() {
                let now = self.clock.now();
                if now >= state_data.borrow().next_transition_at {
                    {
                        let mut s = state_data.borrow_mut();
                        s.next_transition_at = now + FADE_OFF_STEP_DURATION;
                        s.color.step_down();
                    }
                    self.driver.set_pixels(&state_data.borrow().color);
                }
                state_data.borrow().color.is_off()
            } else {
                true
            }
        }
    }

    #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
    pub struct OnStateData {
        pub mode: Mode,
        pub color: RGBW8,
        pub started_at: Instant,
    }

    #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
    pub struct FadeOffStateData {
        pub color: RGBW8,
        pub next_transition_at: Instant,
    }
}
