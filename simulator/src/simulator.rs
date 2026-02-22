use gif::{DisposalMethod, Encoder, Frame};
use image::{ImageBuffer, Rgb};
use std::{collections::HashMap, io::Read};

use super::encoder::{crop, diff_rect, rgb_to_indexed};

const FRAME_DURATION_MS: u16 = 10; // GIF format minimum is 10ms -> 100Hz animation

const PIXEL_BG: Rgb<u8> = Rgb([255, 255, 255]);
const PIXEL_PRINT: Rgb<u8> = Rgb([0, 0, 0]);
const PIXEL_CUR_PAINT: Rgb<u8> = Rgb([255, 0, 0]);
const PIXEL_CUR_TRAVERSE: Rgb<u8> = Rgb([0, 255, 255]);

type Instr = (i32, i32, bool);
type ImgSeq = Vec<ImageBuffer<Rgb<u8>, Vec<u8>>>;

/// Decode RLE bit-packed instruction stream:
///   - 4 bits opcode
///   - 6 bits run length (len - 1)
fn parse_instructions(path: &str) -> Result<Vec<Instr>, String> {
    let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut data = Vec::new();
    file.read_to_end(&mut data).map_err(|e| e.to_string())?;

    let mut op_to_instr: HashMap<u8, Vec<Instr>> = HashMap::new();

    op_to_instr.insert(0b0000, vec![(1, 0, false)]);
    op_to_instr.insert(0b0001, vec![(-1, 0, false)]);
    op_to_instr.insert(0b0010, vec![(0, 1, false)]);
    op_to_instr.insert(0b0011, vec![(0, -1, false)]);
    op_to_instr.insert(0b0100, vec![(0, 0, true)]);

    // Fused MOVE + PRINT
    op_to_instr.insert(0b0101, vec![(1, 0, false), (0, 0, true)]);
    op_to_instr.insert(0b0110, vec![(-1, 0, false), (0, 0, true)]);
    op_to_instr.insert(0b0111, vec![(0, 1, false), (0, 0, true)]);
    op_to_instr.insert(0b1000, vec![(0, -1, false), (0, 0, true)]);

    let mut acc: u64 = 0;
    let mut bits: u32 = 0;
    let mut idx: usize = 0;

    let pull =
        |nbits: u32, acc: &mut u64, bits: &mut u32, idx: &mut usize| -> Result<u64, String> {
            while *bits < nbits {
                if *idx >= data.len() {
                    return Err("Unexpected EOF while decoding".into());
                }
                *acc |= (data[*idx] as u64) << *bits;
                *bits += 8;
                *idx += 1;
            }

            let val = *acc & ((1u64 << nbits) - 1);
            *acc >>= nbits;
            *bits -= nbits;
            Ok(val)
        };

    let mut instrs = Vec::new();

    while idx < data.len() || bits >= 10 {
        let op = pull(4, &mut acc, &mut bits, &mut idx)? as u8;
        let run_len = pull(6, &mut acc, &mut bits, &mut idx)? as usize + 1;

        let seq = op_to_instr
            .get(&op)
            .ok_or_else(|| format!("Invalid opcode {:04b}", op))?;

        for _ in 0..run_len {
            instrs.extend(seq.iter().copied());
        }
    }

    Ok(instrs)
}

fn reconstruct(inpath: &str, cursor_enable: bool) -> Result<ImgSeq, String> {
    let instrs = parse_instructions(inpath)?;

    let width = 320;
    let height = 120;

    let mut base = ImageBuffer::from_pixel(width, height, PIXEL_BG);
    let mut frames = Vec::new();

    let mut cx: i32 = 0;
    let mut cy: i32 = 0;

    for (dx, dy, print) in instrs {
        cx += dx;
        cy += dy;

        if print && cx >= 0 && cy >= 0 && cx < width as i32 && cy < height as i32 {
            base.put_pixel(cx as u32, cy as u32, PIXEL_PRINT);
        }

        let mut frame = base.clone();

        if cursor_enable {
            let color = if print {
                PIXEL_CUR_PAINT
            } else {
                PIXEL_CUR_TRAVERSE
            };

            if cx >= 0 && cy >= 0 && cx < width as i32 && cy < height as i32 {
                frame.put_pixel(cx as u32, cy as u32, color);
            }
        }

        frames.push(frame);
    }

    Ok(frames)
}

pub fn reconstruct_image(inpath: &str, outpath: &str) -> Result<(), String> {
    let frames = reconstruct(inpath, false)?;
    let last = frames.last().ok_or("No frames generated")?;
    last.save(outpath).map_err(|e| e.to_string())?;
    Ok(())
}

const PALETTE: [u8; 12] = [
    255, 255, 255, // 0 white
    0, 0, 0, // 1 black
    255, 0, 0, // 2 red
    0, 255, 255, // 3 blue
];

pub fn reconstruct_video(
    inpath: &str,
    outpath: &str,
    framestep: usize,
    debug: bool,
) -> Result<(), String> {
    const WIDTH: usize = 320;
    const HEIGHT: usize = 120;

    let frames = reconstruct(inpath, true)?;

    let mut file = std::fs::File::create(outpath).map_err(|e| e.to_string())?;
    let mut encoder = Encoder::new(&mut file, WIDTH as u16, HEIGHT as u16, &PALETTE)
        .map_err(|e| e.to_string())?;

    let mut prev: Option<Vec<u8>> = None;

    for img in frames.into_iter().step_by(framestep.max(1)) {
        let rgb = img.into_raw(); // RGB, len = w*h*3
        let curr = rgb_to_indexed(&rgb, &PALETTE); // indexed, len = w*h

        if let Some(prev_pixels) = &prev {
            if let Some((x0, y0, x1, y1)) = diff_rect(prev_pixels, &curr, WIDTH, HEIGHT) {
                let cropped = crop(&curr, WIDTH, x0, y0, x1, y1);

                let mut frame = Frame::from_palette_pixels(
                    (x1 - x0) as u16,
                    (y1 - y0) as u16,
                    cropped,
                    PALETTE,
                    // Some(0),
                    if debug { Some(0) } else { None },
                );

                frame.left = x0 as u16;
                frame.top = y0 as u16;
                frame.delay = FRAME_DURATION_MS / 10;
                frame.dispose = DisposalMethod::Keep;
                // frame.transparent = Some(0); // background color

                encoder.write_frame(&frame).map_err(|e| e.to_string())?;
            }
        } else {
            // First frame must be full
            let mut frame = Frame::from_palette_pixels(
                WIDTH as u16,
                HEIGHT as u16,
                curr.clone(),
                PALETTE,
                None,
            );

            frame.delay = FRAME_DURATION_MS / 10;
            frame.dispose = DisposalMethod::Keep;

            encoder.write_frame(&frame).map_err(|e| e.to_string())?;
        }

        prev = Some(curr);
    }

    Ok(())
}
