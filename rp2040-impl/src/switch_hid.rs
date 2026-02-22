use core::mem::size_of;

#[repr(u8)]
#[derive(defmt::Format)]
#[allow(dead_code)]
pub enum SwitchHat {
    Up = 0x00,
    Upright = 0x01,
    Right = 0x02,
    Downright = 0x03,
    Down = 0x04,
    Downleft = 0x05,
    Left = 0x06,
    Upleft = 0x07,
    Nothing = 0x08,
}

#[repr(u16)]
#[allow(dead_code)]
pub enum SwitchButtons {
    MaskY = 1 << 0,
    MaskB = 1 << 1,
    MaskA = 1 << 2,
    MaskX = 1 << 3,
    MaskL = 1 << 4,
    MaskR = 1 << 5,
    MaskZl = 1 << 6,
    MaskZr = 1 << 7,
    MaskMinus = 1 << 8,
    MaskPlus = 1 << 9,
    MaskLb = 1 << 10,
    MaskRb = 1 << 11,
    MaskHome = 1 << 12,
    MaskCapture = 1 << 13,
}
/* Button report (16 bits) */

/* Joystick ranges */
pub const SWITCH_JOYSTICK_MIN: u8 = 0x00;
pub const SWITCH_JOYSTICK_MID: u8 = 0x80;
#[allow(dead_code)]
pub const SWITCH_JOYSTICK_MAX: u8 = 0xFF;

use bytemuck::{Pod, Zeroable};

#[repr(C, align(2))]
#[derive(Debug, defmt::Format, Copy, Clone, Pod, Zeroable)]
pub struct SwitchReport {
    pub buttons: u16,
    pub hat: u8,
    pub lx: u8,
    pub ly: u8,
    pub rx: u8,
    pub ry: u8,
    pub vendor: u8,
}

impl SwitchReport {
    pub fn new() -> Self {
        Self {
            buttons: 0,
            hat: SwitchHat::Nothing as u8,
            lx: SWITCH_JOYSTICK_MID,
            ly: SWITCH_JOYSTICK_MID,
            rx: SWITCH_JOYSTICK_MID,
            ry: SWITCH_JOYSTICK_MID,
            vendor: 0,
        }
    }

    pub fn to_bytes(self) -> [u8; 8] {
        let b = self.buttons.to_le_bytes();
        [
            b[0],
            b[1],
            self.hat,
            self.lx,
            self.ly,
            self.rx,
            self.ry,
            self.vendor,
        ]
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone, Debug)]
pub struct SwitchOutReport {
    pub buttons: u16,
    pub hat: u8,
    pub lx: u8,
    pub ly: u8,
    pub rx: u8,
    pub ry: u8,
}

/* Compile-time layout checks (optional but recommended) */
const _: () = assert!(size_of::<SwitchReport>() == 8);
const _: () = assert!(size_of::<SwitchOutReport>() == 7);

pub const SWITCH_REPORT_DESCRIPTOR: &[u8] = &[
    0x05, 0x01, // Usage Page (Generic Desktop Ctrls)
    0x09, 0x05, // Usage (Game Pad)
    0xA1, 0x01, // Collection (Application)
    0x15, 0x00, //   Logical Minimum (0)
    0x25, 0x01, //   Logical Maximum (1)
    0x35, 0x00, //   Physical Minimum (0)
    0x45, 0x01, //   Physical Maximum (1)
    0x75, 0x01, //   Report Size (1)
    0x95, 0x10, //   Report Count (16)
    0x05, 0x09, //   Usage Page (Button)
    0x19, 0x01, //   Usage Minimum (0x01)
    0x29, 0x10, //   Usage Maximum (0x10)
    0x81, 0x02, //   Input (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)
    0x05, 0x01, //   Usage Page (Generic Desktop Ctrls)
    0x25, 0x07, //   Logical Maximum (7)
    0x46, 0x3B, 0x01, //   Physical Maximum (315)
    0x75, 0x04, //   Report Size (4)
    0x95, 0x01, //   Report Count (1)
    0x65, 0x14, //   Unit (System: English Rotation, Length: Centimeter)
    0x09, 0x39, //   Usage (Hat switch)
    0x81, 0x42, //   Input (Data,Var,Abs,No Wrap,Linear,Preferred State,Null State)
    0x65, 0x00, //   Unit (None)
    0x95, 0x01, //   Report Count (1)
    0x81, 0x01, //   Input (Const,Array,Abs,No Wrap,Linear,Preferred State,No Null Position)
    0x26, 0xFF, 0x00, //   Logical Maximum (255)
    0x46, 0xFF, 0x00, //   Physical Maximum (255)
    0x09, 0x30, //   Usage (X)
    0x09, 0x31, //   Usage (Y)
    0x09, 0x32, //   Usage (Z)
    0x09, 0x35, //   Usage (Rz)
    0x75, 0x08, //   Report Size (8)
    0x95, 0x04, //   Report Count (4)
    0x81, 0x02, //   Input (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)
    0x06, 0x00, 0xFF, //   Usage Page (Vendor Defined 0xFF00)
    0x09, 0x20, //   Usage (0x20)
    0x95, 0x01, //   Report Count (1)
    0x81, 0x02, //   Input (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position)
    0x0A, 0x21, 0x26, //   Usage (0x2621)
    0x95, 0x08, //   Report Count (8)
    0x91,
    0x02, //   Output (Data,Var,Abs,No Wrap,Linear,Preferred State,No Null Position,Non-volatile)
    0xC0, // End Collection
];
