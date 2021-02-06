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

use panic_abort as _;
use stm32f3xx_hal as hal;

use cortex_m::asm;
use cortex_m_rt::{entry, exception, ExceptionFrame};
use hal::{
    pac,
    prelude::*,
    usb::{Peripheral, UsbBus},
};
use log::warn;
use night_light_lib::*;
use usb_device::prelude::*;
use usbd_serial::{SerialPort, USB_CLASS_CDC};

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
        static mut USB_RX_MEM: [u8; DEFAULT_RX_BUFFER_CAPACITY] = [0; DEFAULT_RX_BUFFER_CAPACITY];
        &mut USB_RX_MEM[..]
    };
    let usb_tx_mem = unsafe {
        static mut USB_TX_MEM: [u8; DEFAULT_TX_BUFFER_CAPACITY] = [0; DEFAULT_TX_BUFFER_CAPACITY];
        &mut USB_TX_MEM[..]
    };

    let usb_bus = UsbBus::new(usb);
    let usb_serial_port = SerialPort::new_with_store(&usb_bus, usb_rx_mem, usb_tx_mem);
    let usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .manufacturer("StuffByJon")
        .product("Night Light")
        .serial_number("0001")
        .device_class(USB_CLASS_CDC)
        .build();

    let mut usb = UsbTransport::new(usb_dev, usb_serial_port);

    // System clock tracking millis, interrupt driven
    SYS_CLOCK.enable_systick_interrupt(cp.SYST, clocks);

    // TODO -setup fast/slow LED blink timer(s)

    loop {
        // TODO - timer for this
        usb.poll();

        if usb.state() == UsbDeviceState::Configured {
            let now = SYS_CLOCK.now();
            if now.as_millis() % 1000 == 0 {
                let bytes = now.as_millis().to_le_bytes();
                if let Err(e) = usb.write(&bytes) {
                    pin_led.toggle().ok();
                    warn!("Failed to write {:?}", e);
                }
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
