#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Xp = 0b000, // dx= 1, dy=0
    Xm = 0b001, // dx=-1, dy=0
    Yp = 0b010, // dx=0, dy= 1
    Ym = 0b011, // dx=0, dy=-1
    D = 0b100,  // print
    XpD = 0b0101,
    XmD = 0b0110,
    YpD = 0b0111,
    YmD = 0b1000,
}

impl Op {
    pub fn prints(&self) -> bool {
        matches!(self, Op::D | Op::XpD | Op::XmD | Op::YpD | Op::YmD)
    }

    pub fn pack(&self) -> Op {
        match self {
            Op::Xp => Op::XpD,
            Op::Xm => Op::XmD,
            Op::Yp => Op::YpD,
            Op::Ym => Op::YmD,
            _ => unreachable!(),
        }
    }
}
