use crate::{
    BasicColor, Button, Duration, InfallibleLedDriver, IrCommand, SystemClock, White, RGBW, RGBW8,
};
use log::debug;
use private::{Context, Events, StateMachine};

// TODO
// brightness handling
// idle state with duration idle for system reset?

const AUTO_ON_DURATION: Duration = Duration::ONE_MINUTE;
const MANUAL_ON_DURATION: Duration = Duration::ONE_MINUTE;
//const AUTO_ON_DURATION: Duration = Duration::TEN_MINUTES;
//const MANUAL_ON_DURATION: Duration = Duration::ONE_HOUR;

// TODO - rm the saturation_sub() methods/etc on instant
const ONOFF_FADE_STEP_DURATION: Duration = Duration::from_millis(10);
const FADE_MODE_STEP_DURATION: Duration = Duration::from_millis(100);

/// Color used for AutoOn and ManualOn
const DEFAULT_COLOR: RGBW8 = RGBW {
    r: 64,
    g: 0,
    b: 0,
    a: White(128),
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
        let maybe_btn_color = BasicColor::from_button(cmd.button);

        // TODO - fixup the repeat checking, only brightness will allow it
        match cmd.button {
            Button::Off if !cmd.repeat => {
                self.sm.process_event(Events::ManualOff).ok();
            }
            Button::On if !cmd.repeat => {
                self.sm.process_event(Events::ManualOn(DEFAULT_COLOR)).ok();
            }
            Button::White if !cmd.repeat => {
                let color = RGBW8::new_alpha(0, 0, 0, White(255));
                self.sm.process_event(Events::ManualOn(color)).ok();
            }
            _btn if !cmd.repeat && maybe_btn_color.is_some() => {
                self.sm
                    .process_event(Events::ManualOn(maybe_btn_color.unwrap().as_rgbw()))
                    .ok();
            }
            Button::Fade if !cmd.repeat => {
                self.sm.process_event(Events::Fade).ok();
            }
            _ => debug!("Ignoring {}", cmd),
        }
    }
}

mod private {
    use super::{
        AUTO_ON_DURATION, DEFAULT_COLOR, FADE_MODE_STEP_DURATION, MANUAL_ON_DURATION,
        ONOFF_FADE_STEP_DURATION,
    };
    use crate::{
        FadeOffRgbw, FadeToRgbw, InfallibleLedDriver, Instant, RandomColorGen, SystemClock,
        COLOR_OFF, RGBW8,
    };
    use core::cell::RefCell;
    use log::debug;
    use smlang::statemachine;

    #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
    pub enum Mode {
        AutoOn,
        ManualOn,
        Fade,
        /*Flash,
         *Smooth,
         *Strobe,
         *Fade, */
    }

    statemachine! {
        *Reset + Init / init_action = Off,

        Off(OffStateData) + AutoOn / off_to_auto_on_action = On,
        Off(OffStateData) + ManualOn(RGBW8) / off_to_manual_on_action = On,
        Off(OffStateData) + Fade / off_to_fade_on_action = On,
        Off(OffStateData) + TimerCheck [off_timer_check_guard] / off_to_off_action = Off,

        On(OnStateData) + ManualOff / on_to_off_action = Off,
        On(OnStateData) + TimerCheck [on_timer_check_guard] / on_to_off_action = Off,

        On(OnStateData) + ManualOn(RGBW8) / on_to_manual_on_action = On,
        On(OnStateData) + AutoOn / on_to_auto_on_action = On,
        On(OnStateData) + Fade / on_to_fade_on_action = On,
    }

    pub struct Context<LED> {
        driver: LED,
        color_gen: RandomColorGen,
        clock: &'static SystemClock,
    }

    impl<LED> Context<LED>
    where
        LED: InfallibleLedDriver,
    {
        pub fn new(driver: LED, clock: &'static SystemClock) -> Self {
            Context {
                driver,
                color_gen: RandomColorGen::new(clock.now().as_millis() as _),
                clock,
            }
        }

        fn common_enter_on(
            &mut self,
            mode: Mode,
            current_color: RGBW8,
            destination_color: RGBW8,
        ) -> OnStateData {
            debug!("Entered On ({:?}) {:?}", mode, destination_color);
            OnStateData {
                mode,
                started_at: self.clock.now(),
                fade_to: FadeToState::new_refcell(
                    current_color,
                    destination_color,
                    self.clock.now(),
                ),
            }
        }
    }

    impl<LED> StateMachineContext for Context<LED>
    where
        LED: InfallibleLedDriver,
    {
        fn init_action(&mut self) -> OffStateData {
            debug!("Initialized LED controller state machine");
            self.driver.set_off();
            FadeToState::new_refcell(COLOR_OFF, COLOR_OFF, self.clock.now())
        }

        fn off_to_auto_on_action(&mut self, state_data: &OffStateData) -> OnStateData {
            self.common_enter_on(Mode::AutoOn, state_data.borrow().color, DEFAULT_COLOR)
        }

        fn off_to_manual_on_action(
            &mut self,
            state_data: &OffStateData,
            event_data: &RGBW8,
        ) -> OnStateData {
            self.common_enter_on(Mode::ManualOn, state_data.borrow().color, *event_data)
        }

        fn off_to_fade_on_action(&mut self, state_data: &OffStateData) -> OnStateData {
            let next_color = self.color_gen.rand_rgb();
            self.common_enter_on(Mode::Fade, state_data.borrow().color, next_color)
        }

        fn off_to_off_action(&mut self, state_data: &OffStateData) -> OffStateData {
            FadeToState::new_refcell(state_data.borrow().color, COLOR_OFF, self.clock.now())
        }

        fn off_timer_check_guard(&mut self, state_data: &OffStateData) -> bool {
            if !state_data.borrow().color.is_off() {
                let dur_since = self
                    .clock
                    .duration_since(state_data.borrow().transitioned_at);

                if dur_since >= ONOFF_FADE_STEP_DURATION {
                    state_data.borrow_mut().transitioned_at = self.clock.now();
                    state_data.borrow_mut().color.step_down();
                    self.driver.set_pixels(&state_data.borrow().color);
                }

                if state_data.borrow().color.is_off() {
                    debug!("Re-seed PRNG");
                    self.driver.set_off();
                    self.color_gen = RandomColorGen::new(self.clock.now().as_millis() as _);
                }
            }

            state_data.borrow().color.is_off()
        }

        fn on_to_manual_on_action(
            &mut self,
            state_data: &OnStateData,
            event_data: &RGBW8,
        ) -> OnStateData {
            self.common_enter_on(
                Mode::ManualOn,
                state_data.fade_to.borrow().color,
                *event_data,
            )
        }

        fn on_to_auto_on_action(&mut self, state_data: &OnStateData) -> OnStateData {
            self.common_enter_on(
                Mode::AutoOn,
                state_data.fade_to.borrow().color,
                DEFAULT_COLOR,
            )
        }

        fn on_to_fade_on_action(&mut self, state_data: &OnStateData) -> OnStateData {
            let next_color = self.color_gen.rand_rgb();
            self.common_enter_on(Mode::Fade, state_data.fade_to.borrow().color, next_color)
        }

        fn on_to_off_action(&mut self, state_data: &OnStateData) -> OffStateData {
            debug!("Entered Off");
            FadeToState::new_refcell(
                state_data.fade_to.borrow().color,
                COLOR_OFF,
                self.clock.now(),
            )
        }

        fn on_timer_check_guard(&mut self, state_data: &OnStateData) -> bool {
            let dest_color_reached = state_data.fade_to.borrow().destination_color_reached();

            if !dest_color_reached {
                let dur_since = self
                    .clock
                    .duration_since(state_data.fade_to.borrow().transitioned_at);

                let should_step = match state_data.mode {
                    Mode::AutoOn | Mode::ManualOn => dur_since >= ONOFF_FADE_STEP_DURATION,
                    Mode::Fade => dur_since >= FADE_MODE_STEP_DURATION,
                };

                if should_step {
                    let mut f = state_data.fade_to.borrow_mut();
                    f.transitioned_at = self.clock.now();
                    f.step_color_to();
                    self.driver.set_pixels(&f.color);
                }

                if state_data.fade_to.borrow().destination_color_reached() {
                    match state_data.mode {
                        Mode::Fade => {
                            let next_color = self.color_gen.rand_rgb();
                            debug!("Next fade color {:?}", next_color);
                            state_data.fade_to.borrow_mut().destination_color = next_color;
                        }
                        _ => (),
                    }
                }
            }

            if state_data.mode == Mode::AutoOn {
                self.clock.duration_since(state_data.started_at) >= AUTO_ON_DURATION
            } else {
                self.clock.duration_since(state_data.started_at) >= MANUAL_ON_DURATION
            }
        }
    }

    #[derive(Clone, PartialEq, Debug)]
    pub struct OnStateData {
        pub mode: Mode,
        pub started_at: Instant,
        pub fade_to: RefCell<FadeToState>,
    }

    pub type OffStateData = RefCell<FadeToState>;

    #[derive(Copy, Clone, PartialEq, Debug)]
    pub struct FadeToState {
        pub color: RGBW8,
        pub destination_color: RGBW8,
        pub transitioned_at: Instant,
    }

    impl FadeToState {
        fn new_refcell(
            color: RGBW8,
            destination_color: RGBW8,
            transitioned_at: Instant,
        ) -> RefCell<Self> {
            RefCell::new(FadeToState {
                color,
                destination_color,
                transitioned_at,
            })
        }

        fn step_color_to(&mut self) {
            self.color.step_to(&self.destination_color);
        }

        fn destination_color_reached(&self) -> bool {
            self.color.destination_reached(&self.destination_color)
        }
    }
}
