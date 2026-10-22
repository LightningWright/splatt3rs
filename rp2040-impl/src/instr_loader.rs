//! Reads the instruction list from the `.instrs` flash section.
//!
//! The linker script (memory.x) allocates a fixed 96 KiB `.instrs` section filled with 0xFF.
//! Instructions can change without recompiling the firmware.
//!
//! Section layout (little endian)
//! offset  size  field
//! 0       4     Magic  b"RLE1"
//! 4       4     Payload length in bytes
//! 8       4     CRC-32 (ISO-HDLC) of the payload
//! 12      4     Number of instructions
//! 16      ..    Payload: the contents of instrs.rle

use crc::{CRC_32_ISO_HDLC, Crc};

const MAGIC: [u8; 4] = *b"RLE1";
const HEADER_LEN: usize = 16;
const CRC: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

#[derive(Debug, defmt::Format)]
pub enum SectionError {
    /// Still the 0xFF filler: `embed_instrs.py` has not been run on this ELF.
    Empty,
    /// Neither filler nor a valid header.
    BadMagic,
    /// The header claims more bytes than the section has.
    BadLength,
    /// The payload does not match its CRC: a partial or corrupted flash write.
    BadCrc,
}

/// The bytes of the `.instrs` section.
fn section_bytes() -> &'static [u8] {
    // Unsafe context is required to get a reference to the bytes in the .instrs section
    // Both symbols are defined by memory.x.
    unsafe extern "C" {
        static __instrs_start: u8;
        static __instrs_end: u8;
    }
    let start = core::ptr::addr_of!(__instrs_start);
    let end = core::ptr::addr_of!(__instrs_end);
    unsafe { core::slice::from_raw_parts(start, end as usize - start as usize) }
}

/// Checks the header and returns the RLE payload embedded in this firmware
pub fn payload() -> Result<&'static [u8], SectionError> {
    let bytes = section_bytes();
    let hdr: &[u8; HEADER_LEN] = bytes.first_chunk().ok_or(SectionError::BadLength)?;

    if hdr.iter().all(|&b| b == 0xFF) {
        return Err(SectionError::Empty);
    }

    if hdr[..4] != MAGIC {
        return Err(SectionError::BadMagic);
    }

    let word = |i: usize| u32::from_le_bytes([hdr[i], hdr[i + 1], hdr[i + 2], hdr[i + 3]]);
    let (len, crc) = (word(4) as usize, word(8));

    let payload = bytes[HEADER_LEN..].get(..len).ok_or(SectionError::BadLength)?;

    if CRC.checksum(payload) != crc {
        return Err(SectionError::BadCrc);
    }
    Ok(payload)
}
