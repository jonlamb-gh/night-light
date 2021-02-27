use core::fmt;
use hal::time::Hertz;
use heapless::{consts::U8, spsc};
use infrared::protocols::nec::Nec16Command;
use infrared::{protocols::nec::Nec16, PeriodicReceiver};

pub const IR_SAMPLE_RATE: Hertz = Hertz(20_000);

pub type IrReceiver<RecvrPin> = PeriodicReceiver<Nec16, RecvrPin>;

pub struct IrCommandQueue(spsc::Queue<IrCommand, U8, u8, spsc::SingleCore>);

impl IrCommandQueue {
    pub const fn new() -> Self {
        IrCommandQueue(spsc::Queue(unsafe { heapless::i::Queue::u8_sc() }))
    }

    pub fn dequeue(&mut self) -> Option<IrCommand> {
        self.0.dequeue()
    }

    pub fn enqueue(&mut self, item: IrCommand) -> Result<(), IrCommand> {
        self.0.enqueue(item)
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct IrCommand {
    pub button: Button,
    pub repeat: bool,
}

impl fmt::Display for IrCommand {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "IrCommand {{ {}, repeat: {} }}",
            self.button, self.repeat
        )
    }
}

impl From<Nec16Command> for IrCommand {
    fn from(c: Nec16Command) -> Self {
        IrCommand {
            button: c.into(),
            repeat: c.repeat,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum Button {
    BrightnessDown,
    BrightnessUp,
    Off,
    On,
    Green,
    Green1,
    Green2,
    Green3,
    Green4,
    Red,
    Red1,
    Red2,
    Red3,
    Red4,
    Blue,
    Blue1,
    Blue2,
    Blue3,
    Blue4,
    White,
    Flash,
    Smooth,
    Strobe,
    Fade,
    Unknown(u8),
}

impl fmt::Display for Button {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<Nec16Command> for Button {
    fn from(c: Nec16Command) -> Self {
        use Button::*;
        match c.cmd {
            4 => BrightnessDown,
            5 => BrightnessUp,
            6 => Off,
            7 => On,
            8 => Green,
            9 => Red,
            10 => Blue,
            11 => White,
            12 => Green1,
            13 => Red1,
            14 => Blue1,
            15 => Flash,
            16 => Green4,
            17 => Red4,
            18 => Blue4,
            19 => Smooth,
            20 => Green2,
            21 => Red2,
            22 => Blue2,
            23 => Strobe,
            24 => Green3,
            25 => Red3,
            26 => Blue3,
            27 => Fade,
            _ => Unknown(c.cmd),
        }
    }
}
