#![no_std]
#![no_main]
// TODO - lints

// links
// https://github.com/stm32-rs/stm32f3xx-hal/tree/master/examples
//
// WS2812 leds
// https://crates.io/crates/smart-leds
// https://github.com/smart-leds-rs/ws2812-spi-rs
// https://docs.rs/smart-leds/0.3.0/smart_leds/
// https://docs.rs/ws2812-spi/0.4.0/ws2812_spi/index.html
// https://github.com/smart-leds-rs/smart-leds-samples/blob/master/stm32f1-examples/examples/stm32f1_ws2812_spi_blink.rs
//
// low power modes
// timer to enter low power / sleep mode, reset system clock on wakeup
// https://github.com/stm32-rs/stm32f3xx-hal/issues/108
//
// persistent configs in flash
// https://docs.rs/eeprom/0.1.0/eeprom/
//
// static usb example in
// https://github.com/stm32-rs/stm32-usbd-examples/blob/master/example-stm32f103c8/examples/serial_interrupt.rs
// need to cleanup the log impl

use panic_abort as _;

use cortex_m::asm;
use cortex_m_rt::{entry, exception, ExceptionFrame};
use hal::{
    gpio::{gpiob::PB9, Floating, Input},
    interrupt, pac,
    prelude::*,
    spi::Spi,
    timer::{self, Timer},
    usb::{Peripheral, UsbBus},
    watchdog::IndependentWatchDog,
};
use infrared::PeriodicReceiver;
use log::{info, warn};
use night_light_lib::*;
use usb_device::prelude::*;
use usbd_serial::{SerialPort, USB_CLASS_CDC};
use ws2812_spi::Ws2812;

static LOGGER: Logger<SerialPortLogger<UsbBus<Peripheral>, &mut [u8], &mut [u8]>> = Logger::new();
static SYS_CLOCK: SystemClock = SystemClock::new();

type IrRecvrPin = PB9<Input<Floating>>;
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
        .sysclk(48.mhz())
        .pclk1(24.mhz())
        .pclk2(24.mhz())
        .freeze(&mut flash.acr);
    assert!(clocks.usbclk_valid());

    let mut iwdg = IndependentWatchDog::new(dp.IWDG);
    iwdg.stop_on_debug(&dp.DBGMCU, false);
    iwdg.start(500.ms());

    let mut gpioa = dp.GPIOA.split(&mut rcc.ahb);
    let mut gpiob = dp.GPIOB.split(&mut rcc.ahb);
    let mut gpioc = dp.GPIOC.split(&mut rcc.ahb);

    let mut led = gpioc
        .pc13
        .into_push_pull_output(&mut gpioc.moder, &mut gpioc.otyper);
    // LED on, active low
    led.set_low().ok();

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

    let led_driver = Ws2812::new_sk6812w(spi);
    let mut led_controller = LedController::new(led_driver);
    led_controller.set_all_off();
    led_controller.update_leds().ok();

    let ir_pin = gpiob
        .pb9
        .into_floating_input(&mut gpiob.moder, &mut gpiob.pupdr);

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

    // Setup the USB serial transport
    // D+ line (PA12) has a pull-up resister.
    // Pull the D+ pin down to send a RESET condition to the USB bus.
    // This forced reset is needed only for development, without it the host
    // will not reset the device on a new firmwar upload.
    let mut usb_dp = gpioa
        .pa12
        .into_push_pull_output(&mut gpioa.moder, &mut gpioa.otyper);
    usb_dp.set_low().unwrap();
    asm::delay(clocks.sysclk().0 / 200);

    let usb_dm = gpioa.pa11.into_af14(&mut gpioa.moder, &mut gpioa.afrh);
    let usb_dp = usb_dp.into_af14(&mut gpioa.moder, &mut gpioa.afrh);

    let usb = Peripheral {
        usb: dp.USB,
        pin_dm: usb_dm,
        pin_dp: usb_dp,
    };

    let usb_rx_mem = unsafe {
        static mut USB_RX_MEM: [u8; DEFAULT_USB_RX_BUFFER_CAPACITY] =
            [0; DEFAULT_USB_RX_BUFFER_CAPACITY];
        &mut USB_RX_MEM[..]
    };
    let usb_tx_mem = unsafe {
        static mut USB_TX_MEM: [u8; DEFAULT_USB_TX_BUFFER_CAPACITY] =
            [0; DEFAULT_USB_TX_BUFFER_CAPACITY];
        &mut USB_TX_MEM[..]
    };

    let usb_bus = UsbBus::new(usb);
    // HACK: make the borrow have a static lifetime
    let usb_bus_borrow: &'static usb_device::bus::UsbBusAllocator<UsbBus<Peripheral>> =
        unsafe { core::mem::transmute::<_, _>(&usb_bus) };
    let usb_serial_port = SerialPort::new_with_store(&usb_bus_borrow, usb_rx_mem, usb_tx_mem);
    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus_borrow, UsbVidPid(0x16c0, 0x27dd))
        .manufacturer("StuffByJon")
        .product("Night Light Debug Logger")
        .serial_number("0001")
        .device_class(USB_CLASS_CDC)
        .build();
    let mut usb_timer = Timer::tim3(dp.TIM3, 100.hz(), clocks, &mut rcc.apb1);

    unsafe {
        LOGGER.set_inner(SerialPortLogger::from(usb_serial_port));
        log::set_logger(&LOGGER).unwrap();
    }
    log::set_max_level(log::LevelFilter::Trace);

    for _ in 0..5000 {
        iwdg.feed();
        if let Some(port) = LOGGER.inner_mut() {
            port.poll(&mut usb_dev);
        }
        asm::delay(clocks.sysclk().0 / 5000);
    }

    // System clock tracking millis, interrupt driven
    SYS_CLOCK.enable_systick_interrupt(cp.SYST, clocks);

    let mut controller = Controller::new(led_controller, &SYS_CLOCK);
    let mut controller_update_timer = Timer::tim4(dp.TIM4, 200.hz(), clocks, &mut rcc.apb1);

    info!("Night light initialized");

    loop {
        iwdg.feed();

        if usb_timer.wait().is_ok() {
            if let Some(port) = LOGGER.inner_mut() {
                port.poll(&mut usb_dev);
            }
        }

        if let Some(cmd) = unsafe { IR_CMD_QUEUE.dequeue() } {
            controller.handle_ir_command(cmd);
        }

        if controller_update_timer.wait().is_ok() {
            controller.update();
        }

        if SYS_CLOCK.is_near_wrap_around() {
            warn!("System clock is near the wrap around, resetting");
            loop {
                if let Some(port) = LOGGER.inner_mut() {
                    port.poll(&mut usb_dev);
                }
            }
        }
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
