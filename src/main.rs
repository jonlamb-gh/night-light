#![no_std]
#![no_main]
// TODO - lints

// links
// https://github.com/stm32-rs/stm32f3xx-hal/tree/master/examples
//
// https://crates.io/crates/smart-leds
// WS2812 leds
// https://github.com/smart-leds-rs/ws2812-spi-rs
// https://github.com/smart-leds-rs/smart-leds-samples/blob/master/stm32f1-examples/examples/stm32f1_ws2812_spi_blink.rs
//
// low power modes
// https://github.com/stm32-rs/stm32f3xx-hal/issues/108
//
// persistent configs in flash
// https://docs.rs/eeprom/0.1.0/eeprom/
//
// ir receiver
// https://docs.rs/infrared/0.10.0/infrared/
//
// static usb example in
// https://github.com/stm32-rs/stm32-usbd-examples/blob/master/example-stm32f103c8/examples/serial_interrupt.rs
// need to cleanup the log impl

use panic_abort as _;
use stm32f3xx_hal as hal;

use cortex_m::asm;
use cortex_m_rt::{entry, exception, ExceptionFrame};
use hal::{
    gpio::{gpiob::PB9, Floating, Input},
    interrupt, pac,
    prelude::*,
    timer::{self, Timer},
    usb::{Peripheral, UsbBus},
};
use heapless::{consts::U8, spsc};
use infrared::{
    protocols::nec::{Nec16, Nec16Command},
    PeriodicReceiver,
};
use log::info;
use night_light_lib::*;
use usb_device::prelude::*;
use usbd_serial::{SerialPort, USB_CLASS_CDC};

static LOGGER: Logger<SerialPortLogger<UsbBus<Peripheral>, &mut [u8], &mut [u8]>> = Logger::new();
static SYS_CLOCK: SystemClock = SystemClock::new();

// stuff for testing the IR receiver
type RecvPin = PB9<Input<Floating>>;
const SAMPLERATE: u32 = 20_000;
static mut TIMER: Option<Timer<pac::TIM2>> = None;
static mut RECEIVER: Option<PeriodicReceiver<Nec16, RecvPin>> = None;
static mut IR_QUEUE: spsc::Queue<Nec16Command, U8, u8, spsc::SingleCore> =
    spsc::Queue(unsafe { heapless::i::Queue::u8_sc() });

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

    let mut gpioa = dp.GPIOA.split(&mut rcc.ahb);
    let mut gpiob = dp.GPIOB.split(&mut rcc.ahb);
    let mut gpioc = dp.GPIOC.split(&mut rcc.ahb);

    let mut led = gpioc
        .pc13
        .into_push_pull_output(&mut gpioc.moder, &mut gpioc.otyper);
    // LED on, active low
    led.set_low().ok();

    let ir_pin = gpiob
        .pb9
        .into_floating_input(&mut gpiob.moder, &mut gpiob.pupdr);

    let mut ir_timer = Timer::tim2(dp.TIM2, SAMPLERATE.hz(), clocks, &mut rcc.apb1);
    ir_timer.listen(timer::Event::Update);

    let ir_recvr = PeriodicReceiver::new(ir_pin, SAMPLERATE);

    unsafe {
        TIMER.replace(ir_timer);
        RECEIVER.replace(ir_recvr);
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

    unsafe {
        LOGGER.set_inner(SerialPortLogger::from(usb_serial_port));
        log::set_logger(&LOGGER).unwrap();
    }
    log::set_max_level(log::LevelFilter::Trace);

    // TODO - use a timer, do an initial poll loop until Configured or timeout
    for _ in 0..5000 {
        if let Some(port) = LOGGER.inner_mut() {
            port.poll(&mut usb_dev);
        }
        asm::delay(clocks.sysclk().0 / 5000);
    }

    // System clock tracking millis, interrupt driven
    SYS_CLOCK.enable_systick_interrupt(cp.SYST, clocks);

    info!("Night light initialized");

    loop {
        if let Some(port) = LOGGER.inner_mut() {
            port.poll(&mut usb_dev);
        }

        if let Some(cmd) = unsafe { IR_QUEUE.dequeue() } {
            led.toggle().ok();
            info!("{:?}", cmd);
        }

        /*
        if SYS_CLOCK.now().as_millis() > last_t.as_millis().wrapping_add(1000) {
            last_t = SYS_CLOCK.now();
            info!("sdf");
        }
        */

        /*
        if usb_dev.state() == UsbDeviceState::Configured {
            let now = SYS_CLOCK.now();
            if now.as_millis() % 1000 == 0 {
                while SYS_CLOCK.now().as_millis() % 1000 == 0 {}
                pin_led.toggle().ok();
                info!("message {}", now);
            }
        }
        */
    }
}

#[exception]
fn SysTick() {
    SYS_CLOCK.inc_from_interrupt();
}

#[interrupt]
fn TIM2() {
    let timer = unsafe { TIMER.as_mut().unwrap() };
    timer.clear_update_interrupt_flag();

    let receiver = unsafe { RECEIVER.as_mut().unwrap() };
    if let Ok(Some(cmd)) = receiver.poll() {
        let _ = unsafe { IR_QUEUE.enqueue(cmd).ok() };
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
