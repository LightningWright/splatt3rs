use defmt::info;
use heapless::Vec;

use crate::switch_hid::{SWITCH_JOYSTICK_MID, SWITCH_JOYSTICK_MIN, SwitchButtons, SwitchHat, SwitchReport};

pub struct InstrInterpreter<'a> {
    stm: InterpreterStm,
    instrs: &'a [Op],
    offs: usize,
    sync_repeat: usize,
}

enum InterpreterStm {
    ControllerSync,
    CursorMove,
    ClearScreen,
    ExecuteInstr,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, defmt::Format)]
pub enum Op {
    Xp = 0b0000, // dx= 1, dy=0
    Xm = 0b0001, // dx=-1, dy=0
    Yp = 0b0010, // dx=0, dy= 1
    Ym = 0b0011, // dx=0, dy=-1
    D = 0b0100,  // print
    XpD = 0b0101,
    XmD = 0b0110,
    YpD = 0b0111,
    YmD = 0b1000,
}

impl Op {
    pub fn prints(&self) -> bool {
        matches!(self, Op::D | Op::XpD | Op::XmD | Op::YpD | Op::YmD)
    }
}

impl TryFrom<u8> for Op {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0b0000 => Op::Xp,
            0b0001 => Op::Xm,
            0b0010 => Op::Yp,
            0b0011 => Op::Ym,
            0b0100 => Op::D,
            0b0101 => Op::XpD,
            0b0110 => Op::XmD,
            0b0111 => Op::YpD,
            0b1000 => Op::YmD,
            _ => return Err(()),
        })
    }
}

pub fn parse_instructions(data: &[u8]) -> Option<Vec<Op, 76800>> {
    let mut acc: u64 = 0;
    let mut bits: u32 = 0;
    let mut idx: usize = 0;

    let pull = |nbits: u32, acc: &mut u64, bits: &mut u32, idx: &mut usize| -> Option<u64> {
        while *bits < nbits {
            if *idx >= data.len() {
                info!("Unexpected EOF while decoding");
                return None;
            }
            *acc |= (data[*idx] as u64) << *bits;
            *bits += 8;
            *idx += 1;
        }

        let val = *acc & ((1u64 << nbits) - 1);
        *acc >>= nbits;
        *bits -= nbits;
        Some(val)
    };

    let mut instrs: Vec<Op, 76800> = Vec::new();

    while idx < data.len() || bits >= 10 {
        let op = pull(4, &mut acc, &mut bits, &mut idx)? as u8;
        let run_len = pull(6, &mut acc, &mut bits, &mut idx)? as usize + 1;

        let op: Op = op.try_into().unwrap();

        for _ in 0..run_len {
            instrs.push(op).ok()?;
        }
    }

    Some(instrs)
}

impl<'a> InstrInterpreter<'a> {
    pub fn new(instrs: &'a [Op]) -> Self {
        Self {
            stm: InterpreterStm::ControllerSync,
            instrs,
            offs: 0,
            sync_repeat: 100,
        }
    }

    pub fn step(&mut self) -> Option<SwitchReport> {
        let mut report = None;

        while report.is_none() {
            match self.stm {
                InterpreterStm::ControllerSync => {
                    if self.sync_repeat == 0 {
                        self.sync_repeat = 160;
                        self.stm = InterpreterStm::CursorMove;
                    }
                    self.sync_repeat -= 1;
                    report = Some(SwitchReport {
                        buttons: SwitchButtons::MaskA as u16,
                        ..SwitchReport::new()
                    });
                }
                InterpreterStm::CursorMove => {
                    if self.sync_repeat == 0 {
                        self.stm = InterpreterStm::ClearScreen;
                    }
                    self.sync_repeat -= 1;
                    report = Some(SwitchReport {
                        buttons: SwitchButtons::MaskL as u16,
                        lx: SWITCH_JOYSTICK_MIN,
                        ly: SWITCH_JOYSTICK_MIN,
                        ..SwitchReport::new()
                    });
                }
                InterpreterStm::ClearScreen => {
                    self.stm = InterpreterStm::ExecuteInstr;
                    report = Some(SwitchReport {
                        buttons: SwitchButtons::MaskLb as u16,
                        ..SwitchReport::new()
                    });
                }
                InterpreterStm::ExecuteInstr => {
                    if self.offs >= self.instrs.len() {
                        break;
                    }

                    let instr = self.instrs[self.offs];
                    self.offs += 1;

                    report = Some(decode_op(&instr));
                    break;
                }
            }
        }

        report
    }
}

fn decode_op(instr: &Op) -> SwitchReport {
    let mut report = SwitchReport {
        buttons: 0,
        hat: 0,
        lx: SWITCH_JOYSTICK_MID,
        ly: SWITCH_JOYSTICK_MID,
        rx: SWITCH_JOYSTICK_MID,
        ry: SWITCH_JOYSTICK_MID,
        vendor: 0,
    };

    report.hat = match instr {
        Op::Xp => SwitchHat::Right, // Move right
        Op::Xm => SwitchHat::Left,
        Op::Yp => SwitchHat::Down,
        Op::Ym => SwitchHat::Up,
        Op::D => {
            report.buttons = SwitchButtons::MaskA as u16;
            SwitchHat::Nothing
        }
        Op::XpD => {
            report.buttons = SwitchButtons::MaskA as u16;
            SwitchHat::Right
        }
        Op::XmD => {
            report.buttons = SwitchButtons::MaskA as u16;
            SwitchHat::Left
        }
        Op::YpD => {
            report.buttons = SwitchButtons::MaskA as u16;
            SwitchHat::Down
        }
        Op::YmD => {
            report.buttons = SwitchButtons::MaskA as u16;
            SwitchHat::Up
        }
    } as u8;

    report
}
