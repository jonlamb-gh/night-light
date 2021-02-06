# night-light

## Build the Firmware

```bash
cargo build
```

## Flash the Firmware

Dependencies:

* [st-link](https://github.com/texane/stlink) : `sudo apt-get install stlink-tools`
* [OpenOCD](http://openocd.org/getting-openocd/) : `sudo apt-get install openocd`

NOTE: the two jumpers on `boot0` and `boot1` select RAM or FLASH.

* Both tied to `0` == Flash
* Both tied to `1` == System memory

Make sure to tie both jumpers to `0`.

**WARNING**: make sure the USB micro cable is disconnected while powering
the board with the st-link programmer!

If you are powering the board via the USB connection, then don't connect the
st-link's 3.3V pin.

Connect USB st-link to the the Black Pill board:

| st-link | Black Pill |
| :---    |       ---: |
| PIN 2 SWDIO  | DIO |
| PIN 4 GND    | GND |
| PIN 6 SWCLK  | SCK |
| PIN 8 3.3V   | 3V3 |

Plug in the st-link to the host, this will power up the Black Pill board.

Build and flash the firmware:

```bash
# Release build then upload
# Jumpers should both be tied to 0
./flash-firmware
```

To flash a pre-built ELF binary:

```bash
openocd -f openocd.cfg -c "program /path/to/binary verify reset"
```

### Debug/stdout

The debug serial port is connected to USART1, PB6 Tx, PB7 Rx.

```bash
stty -F /dev/ttyUSB0 115200
cat /dev/ttyUSB0
```

```bash
cargo run
```

## Build/Run the Tests

```bash
# cargo test --target x86_64-unknown-linux-gnu --lib
./run-tests
```

## Hardware

* Board: [STM32 Black Pill Development Board](https://robotdyn.com/stm32f303cct6-256-kb-flash-stm32-arm-cortexr-m4-mini-system-dev-board-3326a9dd-3c19-11e9-910a-901b0ebb3621.html)
  - Refman: [STM32F303CCT6](https://www.st.com/content/ccc/resource/technical/document/reference_manual/4a/19/6e/18/9d/92/43/32/DM00043574.pdf/files/DM00043574.pdf/jcr:content/translations/en.DM00043574.pdf)
  - Datasheet: [STM32F303xC](https://www.st.com/resource/en/datasheet/stm32f303cb.pdf)
  - Pinout: [link](https://robotdyn.com/pub/media/GR-00000345==STM32F303CCT6-256KB-STM32MiniSystem/DOCS/PINOUT==GR-00000345==STM32F303CCT6-256KB-STM32MiniSystem.jpg)
  - Schematic: [link](https://robotdyn.com/pub/media/GR-00000345==STM32F303CCT6-256KB-STM32MiniSystem/DOCS/Schematic==GR-00000345==STM32F303CCT6-256KB-STM32MiniSystem.pdf)
* TODO

