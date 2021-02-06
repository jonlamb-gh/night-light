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
    pac,
    prelude::*,
    usb::{Peripheral, UsbBus},
};
use log::{info, warn};
use night_light_lib::*;
use usb_device::prelude::*;
use usbd_serial::{SerialPort, USB_CLASS_CDC};

static LOGGER: Logger<SerialPortLogger<UsbBus<Peripheral>, &mut [u8], &mut [u8]>> = Logger::new();
static SYS_CLOCK: SystemClock = SystemClock::new();

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
    let mut gpioc = dp.GPIOC.split(&mut rcc.ahb);
    let mut pin_led = gpioc
        .pc13
        .into_push_pull_output(&mut gpioc.moder, &mut gpioc.otyper);

    // LED on, active low
    pin_led.set_low().ok();

    // Setup the USB serial transport
    // D+ line (PA12) has a pull-up resister.
    // Pull the D+ pin down to send a RESET condition to the USB bus.
    // This forced reset is needed only for development, without it host
    // will not reset your device when you upload new firmware.
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

        // it works
        if usb_dev.state() == UsbDeviceState::Configured {
            let now = SYS_CLOCK.now();
            if now.as_millis() % 1000 == 0 {
                while SYS_CLOCK.now().as_millis() % 1000 == 0 {}
                pin_led.toggle().ok();
                info!("message {}", now);
            }
        }
    }
}

#[exception]
fn SysTick() {
    SYS_CLOCK.inc_from_interrupt();
}

#[exception]
fn HardFault(ef: &ExceptionFrame) -> ! {
    panic!("HardFault at {:#?}", ef);
}

#[exception]
fn DefaultHandler(irqn: i16) {
    panic!("Unhandled exception (IRQn = {})", irqn);
}
