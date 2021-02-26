#![no_std]
#![no_main]
// TODO - lints

use panic_abort as _;

use cortex_m::asm;
use cortex_m_rt::{entry, exception, ExceptionFrame};
use hal::{
    gpio::{gpioa::PA15, Floating, Input},
    interrupt, pac,
    prelude::*,
    serial::{Serial, Tx},
    spi::Spi,
    timer::{self, Timer},
    watchdog::IndependentWatchDog,
};
use infrared::PeriodicReceiver;
use log::{info, warn};
use night_light_lib::*;
use ws2812_spi::Ws2812;

static GLOBAL_LOGGER: Logger<Tx<pac::USART1>> = Logger::new();

static SYS_CLOCK: SystemClock = SystemClock::new();

type IrRecvrPin = PA15<Input<Floating>>;
static mut IR_TIMER: Option<Timer<pac::TIM2>> = None;
static mut IR_RECVR: Option<IrReceiver<IrRecvrPin>> = None;
static mut IR_CMD_QUEUE: IrCommandQueue = IrCommandQueue::new();

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().expect("Failed to take pac::Peripherals");
    let cp =
        cortex_m::peripheral::Peripherals::take().expect("Failed to take cortex_m::Peripherals");

    // Setup system clock
    let mut flash = dp.FLASH.constrain();
    let mut rcc = dp.RCC.constrain();
    let clocks = rcc
        .cfgr
        .use_hse(8.mhz())
        .sysclk(72.mhz())
        .pclk1(24.mhz())
        .pclk2(24.mhz())
        .freeze(&mut flash.acr);
    assert!(clocks.usbclk_valid());

    let mut iwdg = IndependentWatchDog::new(dp.IWDG);
    iwdg.stop_on_debug(&dp.DBGMCU, false);
    iwdg.start(1000.ms());

    // System clock tracking milliseconds, interrupt driven
    SYS_CLOCK.enable_systick_interrupt(cp.SYST, clocks);

    let mut gpioa = dp.GPIOA.split(&mut rcc.ahb);
    let mut gpiob = dp.GPIOB.split(&mut rcc.ahb);
    let mut gpioc = dp.GPIOC.split(&mut rcc.ahb);

    let mut led = gpioc
        .pc13
        .into_push_pull_output(&mut gpioc.moder, &mut gpioc.otyper);
    // LED on, active low
    led.set_low().ok();

    // Setup USART1 for the logger impl
    let uart_tx = gpiob.pb6.into_af7(&mut gpiob.moder, &mut gpiob.afrl);
    let uart_rx = gpiob.pb7.into_af7(&mut gpiob.moder, &mut gpiob.afrl);

    let serial = Serial::usart1(
        dp.USART1,
        (uart_tx, uart_rx),
        115_200.bps(),
        clocks,
        &mut rcc.apb2,
    );

    // Construct a log impl over the transmitter
    let (tx, _rx) = serial.split();
    unsafe {
        GLOBAL_LOGGER.set_inner(tx);
        log::set_logger(&GLOBAL_LOGGER).unwrap();
    }
    log::set_max_level(log::LevelFilter::Trace);

    let spi_pins = {
        let sck = gpiob.pb3.into_af5(&mut gpiob.moder, &mut gpiob.afrl);
        let miso = gpiob.pb4.into_af5(&mut gpiob.moder, &mut gpiob.afrl);
        let mosi = gpiob.pb5.into_af5(&mut gpiob.moder, &mut gpiob.afrl);
        (sck, miso, mosi)
    };

    let spi = Spi::spi1(
        dp.SPI1,
        spi_pins,
        ws2812_spi::MODE,
        3.mhz(),
        clocks,
        &mut rcc.apb2,
    );

    let mut led_driver = InfallibleSk6812w::from(Ws2812::new_sk6812w(spi));
    led_driver.set_off();

    let ir_pin = gpioa
        .pa15
        .into_floating_input(&mut gpioa.moder, &mut gpioa.pupdr);

    let mut ir_timer = Timer::tim2(dp.TIM2, IR_SAMPLE_RATE, clocks, &mut rcc.apb1);
    ir_timer.listen(timer::Event::Update);

    let ir_recvr = PeriodicReceiver::new(ir_pin, IR_SAMPLE_RATE.0);

    unsafe {
        IR_TIMER.replace(ir_timer);
        IR_RECVR.replace(ir_recvr);
    }

    pac::NVIC::unpend(interrupt::TIM2);
    unsafe {
        pac::NVIC::unmask(interrupt::TIM2);
    };

    let mut controller = Controller::new(led_driver, &SYS_CLOCK);
    let mut controller_update_timer = Timer::tim4(dp.TIM4, 200.hz(), clocks, &mut rcc.apb1);

    info!("Night light initialized");

    loop {
        iwdg.feed();

        if let Some(cmd) = unsafe { IR_CMD_QUEUE.dequeue() } {
            led.toggle().ok();
            controller.handle_ir_command(cmd);
        }

        if controller_update_timer.wait().is_ok() {
            controller.update();
        }

        if SYS_CLOCK.is_near_wrap_around() {
            warn!("System clock is near the wrap around, resetting");
            loop {
                asm::nop();
            }
        }

        asm::wfi();
    }
}

#[exception]
fn SysTick() {
    SYS_CLOCK.inc_from_interrupt();
}

#[interrupt]
fn TIM2() {
    // Unsafe ok, timer and recvr only used in this handler
    let timer = unsafe { IR_TIMER.as_mut().unwrap() };
    timer.clear_update_interrupt_flag();

    let recvr = unsafe { IR_RECVR.as_mut().unwrap() };
    if let Ok(Some(cmd)) = recvr.poll() {
        let _ = unsafe { IR_CMD_QUEUE.enqueue(cmd.into()).ok() };
    }
}

#[exception]
fn HardFault(ef: &ExceptionFrame) -> ! {
    panic!("HardFault at {:#?}", ef);
}

#[exception]
fn DefaultHandler(irqn: i16) {
    panic!("Unhandled exception (IRQn = {})", irqn);
}
