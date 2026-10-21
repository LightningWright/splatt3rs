//! Simulate an instruction file as a picture or an animated GIF.
//!
//! # Instruction file
//! A bit-packed stream of run-length-coded words, each 10 bits, least significant bit first:
//!
//! ```
//! 4 bits  opcode      0..=3 move right/left/down/up
//!                     4     print
//!                     5..=8 move right/left/down/up, then print
//! 6 bits  run length  repeat count minus one
//! ```
//!
//! # Simulation
//! The cursor starts at (0,0) on a 320x120 white canvas. Print instruction fills the cell under it.
//!
//! # Recording the animation
//! The GIF is built from *frames* that cover only what changed. A configurable frame skip is
//! available, merging the changes from multiple frames into one.
//!
//! * Each frame is cropped to the bounding box of the cells that changed
//! * A frame with no change just extends the previous frame's delay.
//! * In debug mode, white is the transparent colour, so the cursor leaves a permanent coloured
//!   trail on the canvas.
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    iter,
    ops::{Index, IndexMut},
};

use image::{ImageBuffer, Rgb};

use super::encoder::{GifWriter, Rect};

/// Canvas width and height
const WIDTH: usize = 320;
const HEIGHT: usize = 120;

/// Target frames per second for the GIF animation
const FPS: u16 = 50;

/// Delay of one frame in GIF units (1/100 s).
const FRAME_DELAY_CS: u16 = 100 / FPS;

/// Bits of the opcode field of a token.
const RLE_OPCODE_BITS: u32 = 4;
/// Bits of the run-length field of a token.
const RLE_LEN_BITS: u32 = 6;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum Color {
    /// An empty cell.
    White,
    /// A printed cell.
    Black,
    /// The cursor while printing.
    Red,
    /// The cursor while moving.
    Cyan,
}

impl Color {
    /// RGB values in palette order.
    /// Four colours mean 2-bit LZW codes, the smallest the GIF format allows.
    const PALETTE: [[u8; 3]; 4] = [[255, 255, 255], [0, 0, 0], [255, 0, 0], [0, 255, 255]];

    fn rgb(self) -> Rgb<u8> {
        Rgb(Self::PALETTE[self as usize])
    }
}

/// A direction of the cursor.
#[derive(Clone, Copy)]
pub enum Direction {
    Right,
    Left,
    Down,
    Up,
}

impl Direction {
    /// The unit step `(dx, dy)` of this direction.
    fn delta(self) -> (i32, i32) {
        match self {
            Direction::Right => (1, 0),
            Direction::Left => (-1, 0),
            Direction::Down => (0, 1),
            Direction::Up => (0, -1),
        }
    }
}

/// One decoded instruction.
#[derive(Clone, Copy)]
pub enum Instr {
    /// Move the cursor one cell.
    Move(Direction),
    /// Fill the cell under the cursor.
    Print,
}

impl Instr {
    /// How far the cursor moves: a unit step for a move, nothing for a print.
    pub fn delta(self) -> (i32, i32) {
        match self {
            Instr::Move(direction) => direction.delta(),
            Instr::Print => (0, 0),
        }
    }

    /// Whether this instruction prints.
    pub fn is_print(self) -> bool {
        matches!(self, Instr::Print)
    }
}

/// Reads fields of arbitrary bit width from a byte stream, least significant bit first.
struct BitReader<'a> {
    /// Bytes not consumed yet.
    bytes: iter::Copied<std::slice::Iter<'a, u8>>,
    /// Bits taken from `bytes` but not read yet, lowest bit first.
    buffer: u64,
    /// Number of valid bits in `buffer`.
    buffered: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        BitReader {
            bytes: data.iter().copied(),
            buffer: 0,
            buffered: 0,
        }
    }

    /// Whether fewer bits remain than one full instruction word
    fn at_end(&self) -> bool {
        self.bytes.len() == 0 && self.buffered < RLE_OPCODE_BITS + RLE_LEN_BITS
    }

    /// Reads the next `nbits` bits as a number.
    fn read(&mut self, nbits: u32) -> Result<usize, String> {
        while self.buffered < nbits {
            let byte = self.bytes.next().ok_or("Unexpected EOF while decoding")?;
            self.buffer |= u64::from(byte) << self.buffered;
            self.buffered += 8;
        }
        let value = self.buffer & ((1 << nbits) - 1);
        self.buffer >>= nbits;
        self.buffered -= nbits;
        Ok(value as usize)
    }
}

/// The instructions that an opcode stands for.
fn expand(opcode: usize) -> Result<&'static [Instr], String> {
    use Direction::{Down, Left, Right, Up};
    Ok(match opcode {
        0 => &[Instr::Move(Right)],
        1 => &[Instr::Move(Left)],
        2 => &[Instr::Move(Down)],
        3 => &[Instr::Move(Up)],
        4 => &[Instr::Print],
        5 => &[Instr::Move(Right), Instr::Print],
        6 => &[Instr::Move(Left), Instr::Print],
        7 => &[Instr::Move(Down), Instr::Print],
        8 => &[Instr::Move(Up), Instr::Print],
        _ => return Err(format!("Invalid opcode {opcode:04b}")),
    })
}

/// Decodes an instruction file into a list of instructions.
pub fn parse_instructions(path: &str) -> Result<Vec<Instr>, String> {
    let data = fs::read(path).map_err(|e| e.to_string())?;
    let mut reader = BitReader::new(&data);
    let mut instrs = Vec::new();

    while !reader.at_end() {
        let opcode = reader.read(RLE_OPCODE_BITS)?;
        let run_len = reader.read(RLE_LEN_BITS)? + 1;
        instrs.extend(iter::repeat_n(expand(opcode)?, run_len).flatten());
    }
    Ok(instrs)
}

/// A cell inside the canvas.
#[derive(Clone, Copy)]
struct Cell {
    x: usize,
    y: usize,
}

impl Cell {
    /// The cell at signed coordinates, or `None` if they are off the canvas.
    fn at(x: i32, y: i32) -> Option<Self> {
        let (x, y) = (usize::try_from(x).ok()?, usize::try_from(y).ok()?);
        (x < WIDTH && y < HEIGHT).then_some(Cell { x, y })
    }

    /// Every cell of the canvas in row-major order (the order of `Canvas`).
    fn all() -> impl Iterator<Item = Cell> {
        (0..HEIGHT).flat_map(|y| (0..WIDTH).map(move |x| Cell { x, y }))
    }
}

impl<T> Index<Cell> for Canvas<T> {
    type Output = T;

    fn index(&self, cell: Cell) -> &T {
        &self.0[cell.y * WIDTH + cell.x]
    }
}

impl<T> IndexMut<Cell> for Canvas<T> {
    fn index_mut(&mut self, cell: Cell) -> &mut T {
        &mut self.0[cell.y * WIDTH + cell.x]
    }
}

/// One value per canvas cell, indexed by [`Cell`].
struct Canvas<T>(Vec<T>);

impl<T: Clone> Canvas<T> {
    /// A canvas with every cell set to `value`.
    fn filled(value: T) -> Self {
        Canvas(vec![value; WIDTH * HEIGHT])
    }
}

impl<T> Canvas<T> {
    // Apply a function to every cell
    fn map(f: impl FnMut(Cell) -> T) -> Self {
        Canvas(Cell::all().map(f).collect())
    }
}

/// An inclusive rectangle of cells.
#[derive(Clone, Copy)]
struct Region {
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
}

impl Region {
    /// The smallest region containing all `cells`, or `None` if there are none.
    fn enclosing(cells: impl IntoIterator<Item = Cell>) -> Option<Self> {
        cells.into_iter().fold(None, |region, cell| {
            Some(match region {
                None => Region {
                    x0: cell.x,
                    y0: cell.y,
                    x1: cell.x,
                    y1: cell.y,
                },
                Some(r) => Region {
                    x0: r.x0.min(cell.x),
                    y0: r.y0.min(cell.y),
                    x1: r.x1.max(cell.x),
                    y1: r.y1.max(cell.y),
                },
            })
        })
    }

    /// The cells of the region in row-major order.
    fn cells(self) -> impl Iterator<Item = Cell> {
        (self.y0..=self.y1).flat_map(move |y| (self.x0..=self.x1).map(move |x| Cell { x, y }))
    }

    /// The region as a GIF frame rectangle.
    fn rect(self) -> Rect {
        Rect {
            left: self.x0,
            top: self.y0,
            width: self.x1 - self.x0 + 1,
            height: self.y1 - self.y0 + 1,
        }
    }
}

/// The cursor's position. It may wander off the canvas.
#[derive(Default)]
struct Cursor {
    x: i32,
    y: i32,
}

impl Cursor {
    /// Execute the instruction, and return the cell under it, if that cell is inside the canvas.
    fn apply(&mut self, instr: Instr) -> Option<Cell> {
        let (dx, dy) = instr.delta();
        self.x += dx;
        self.y += dy;
        Cell::at(self.x, self.y)
    }
}

/// Replays instructions and turns snapshots of the state into GIF frames.
struct Recorder<W: Write> {
    pub gif: GifWriter<W>,
    /// Debug mode: white is transparent, so the cursor's trail stays visible.
    debug: bool,
    /// Whether the first (full-canvas) frame has been written.
    started: bool,
    cursor: Cursor,
    /// The true picture, without the cursor.
    base: Canvas<Color>,
    /// What a GIF viewer displays after the frames written so far.
    shown: Canvas<Color>,
    /// Cursor colour on the cells it stood on since the last frame; overlays `base`
    overlay: Canvas<Option<Color>>,
    /// Cells the cursor stood on since the last frame, in visiting order.
    trail: Vec<Cell>,
    /// The previous frame's `trail`. Those cells fall back to `base` in the next frame, and are in need of a redraw
    erased: Vec<Cell>,
}

impl<W: Write> Recorder<W> {
    /// A recorder that writes to `gif`, starting from a blank canvas.
    fn new(gif: GifWriter<W>, debug: bool) -> Self {
        Recorder {
            gif,
            debug,
            started: false,
            cursor: Cursor::default(),
            base: Canvas::filled(Color::White),
            shown: Canvas::filled(Color::White),
            overlay: Canvas::filled(None),
            trail: Vec::new(),
            erased: Vec::new(),
        }
    }

    /// Executes an instruction and remembers the cell the cursor stands on for the next frame.
    fn apply(&mut self, instr: Instr) {
        let Some(cell) = self.cursor.apply(instr) else {
            return; // off the canvas: nothing to draw
        };

        if instr.is_print() {
            self.base[cell] = Color::Black;
        }

        let cursor_color = if instr.is_print() { Color::Red } else { Color::Cyan };

        self.overlay[cell] = Some(cursor_color);
        self.trail.push(cell);
    }

    /// The colour a cell should have in the frame being written.
    fn desired(&self, cell: Cell) -> Color {
        self.overlay[cell].unwrap_or(self.base[cell])
    }

    /// Whether `color` has to be written to the cell because the viewer is showing a different one.
    /// In Debug mode White color is considered transparent
    fn changed(&self, cell: Cell, color: Color) -> bool {
        color != self.shown[cell] && !(self.debug && color == Color::White)
    }

    /// Writes the current changes into a full or partial frame and starts a new one.
    fn snapshot(&mut self) -> Result<(), String> {
        if self.started {
            self.write_changes()?;
        } else {
            self.write_full()?;
            self.started = true;
        }

        // Frame done: drop the trail. Its cells fall back to `base` in the next frame.
        for &cell in &self.trail {
            self.overlay[cell] = None;
        }
        self.erased = std::mem::take(&mut self.trail);
        Ok(())
    }

    /// Writes the first frame, which covers the whole canvas.
    fn write_full(&mut self) -> Result<(), String> {
        self.shown = Canvas::map(|cell| self.desired(cell));
        let full = Region {
            x0: 0,
            y0: 0,
            x1: WIDTH - 1,
            y1: HEIGHT - 1,
        };
        let pixels = self.shown.0.iter().map(|&color| color as u8);
        self.gif.push(full.rect(), pixels, None)
    }

    /// Writes a frame with only the bounding box of the cells that changed
    fn write_changes(&mut self) -> Result<(), String> {
        // Only cells the cursor touched now or in the previous frame can differ.
        let candidates = self.erased.iter().chain(&self.trail).copied();
        let changed = candidates.filter(|&cell| self.changed(cell, self.desired(cell)));

        let Some(region) = Region::enclosing(changed) else {
            self.gif.hold(); // Nothing visible changed
            return Ok(());
        };

        // Every cell of the region with its colour and whether it is a change.
        let cells: Vec<(Cell, Color, bool)> = region
            .cells()
            .map(|cell| {
                let color = self.desired(cell);
                (cell, color, self.changed(cell, color))
            })
            .collect();

        let debug = self.debug;
        let pixels = cells.iter().map(|&(_, color, changed)| {
            if debug && !changed {
                Color::White as u8 // White == No changes == Transparent
            } else {
                color as u8
            }
        });

        let transparent = debug.then_some(Color::White as u8);
        self.gif.push(region.rect(), pixels, transparent)?;

        for (cell, color, _) in cells.into_iter().filter(|&(.., changed)| changed) {
            self.shown[cell] = color;
        }
        Ok(())
    }
}

/// Replays an instruction file as an animated GIF
pub fn reconstruct_video(inpath: &str, outpath: &str, framestep: usize, debug: bool) -> Result<(), String> {
    let instrs = parse_instructions(inpath)?;

    if instrs.is_empty() {
        return Err("No frames generated".into());
    }

    let step = framestep.max(1);
    let last = instrs.len() - 1;

    let file = BufWriter::new(File::create(outpath).map_err(|e| e.to_string())?);
    let gif = GifWriter::new(file, WIDTH, HEIGHT, Color::PALETTE.as_flattened(), FRAME_DELAY_CS)?;

    // With `debug` option set, the cursor leaves a permanent coloured trail.
    let mut recorder = Recorder::new(gif, debug);

    for (i, &instr) in instrs.iter().enumerate() {
        recorder.apply(instr);

        // One frame is written for every `framestep` instructions (and for the last one).
        if i % step == 0 || i == last {
            recorder.snapshot()?;
        }
    }
    recorder.gif.finish()
}

/// Renders the final picture of an instruction file as an image.
pub fn reconstruct_image(inpath: &str, outpath: &str) -> Result<(), String> {
    let instrs = parse_instructions(inpath)?;

    if instrs.is_empty() {
        return Err("No frames generated".into());
    }

    // Only the final picture is needed, so no per-step frames are kept.
    let mut picture = Canvas::filled(Color::White);
    let mut cursor = Cursor::default();

    for instr in instrs {
        let cell = cursor.apply(instr);

        if instr.is_print()
            && let Some(cell) = cell
        {
            picture[cell] = Color::Black;
        }
    }

    ImageBuffer::from_fn(WIDTH as u32, HEIGHT as u32, |x, y| {
        let cell = Cell {
            x: x as usize,
            y: y as usize,
        };

        picture[cell].rgb()
    })
    .save(outpath)
    .map_err(|e| e.to_string())
}
