//! Grid Algorithm
//!
//! Generates a grid of fixed size square blocks and visits them in a serpentine order (left to right on one block row,
//! right to left on the next).
//! Inside each block the pixels are scanned in a serpentine too, choosing the best of 8 orientations (rows or columns,
//! each with a flipped outer and/or inner direction) for the current cursor position.
//!
//! Several block sizes are tried in parallel and the best result wins.
//!
//! Blocks keep the cursor local, so it beats a plain line sweep on sparse
//! images, but it has no notion of the actual shape of the pixels.

use std::{collections::HashSet, iter};

use rayon::prelude::*;

use crate::{
    Op, Point,
    algorithms::{
        AlgorithmSettings,
        tsp_core::{Bounds, build_ops, manhattan},
    },
};

/// Block side lengths that are tried.
const BLOCK_SIZES: [i32; 11] = [2, 3, 4, 5, 6, 8, 10, 16, 20, 40, 120];

/// How a block is scanned: one of 8 serpentine orientations.
#[derive(Clone, Copy)]
struct Orientation {
    /// Scan along rows (otherwise along columns).
    by_rows: bool,
    /// Start the first line in the opposite direction.
    flip_inner: bool,
    /// Visit the lines of the block in reverse order.
    flip_outer: bool,
}

impl Orientation {
    /// All 8 combinations.
    fn all() -> impl Iterator<Item = Self> {
        (0..8u8).map(|bits| Orientation {
            by_rows: bits & 1 != 0,
            flip_outer: bits & 2 != 0,
            flip_inner: bits & 4 != 0,
        })
    }
}

/// Tries every block size in parallel and returns the instructions of the shortest result
pub fn alg(pixels: HashSet<Point>, _: AlgorithmSettings) -> Vec<Op> {
    let Some(bounds) = Bounds::of(pixels.iter().copied()) else {
        return Vec::new();
    };

    BLOCK_SIZES
        .into_par_iter()
        .map(|size| build_ops(&pixels, &scan(&pixels, bounds, size)))
        .min_by_key(|ops| ops.len())
        .expect("at least one block size is tried")
}

/// Scan blocks to generate an optimal visiting order
fn scan(pixels: &HashSet<Point>, bounds: Bounds, size: i32) -> Vec<Point> {
    let columns = (bounds.width() + size - 1) / size;
    let mut order = Vec::with_capacity(pixels.len());
    let mut cursor = Point(0, 0);

    for (row, y0) in bounds.ys().step_by(size as usize).enumerate() {
        for column in 0..columns {
            // Odd block rows run right to left.
            let column = if row % 2 == 1 { columns - 1 - column } else { column };
            let x0 = bounds.x0 + column * size;
            let block = Bounds {
                x0,
                y0,
                x1: (x0 + size - 1).min(bounds.x1),
                y1: (y0 + size - 1).min(bounds.y1),
            };

            // Compare path traversals to select the shortest one
            let best = Orientation::all()
                .map(|orientation| block_path(pixels, block, orientation))
                .filter(|path| !path.is_empty())
                .min_by_key(|path| travel(cursor, path));

            if let Some(path) = best {
                cursor = path[path.len() - 1];
                order.extend(path);
            }
        }
    }
    order
}

/// Distance travelled by walking from `from` to the first point of `path` and then along it.
fn travel(from: Point, path: &[Point]) -> i32 {
    iter::once(&from)
        .chain(path)
        .zip(path)
        .map(|(&a, &b)| manhattan(a, b))
        .sum()
}

/// Maps position `k` in `0..len` to its position when the range is walked backwards `flip == true`,
fn mirrored(k: i32, len: i32, flip: bool) -> i32 {
    if flip { len - 1 - k } else { k }
}

/// Returns the pixels inside `block` in serpentine order for the given orientation
fn block_path(pixels: &HashSet<Point>, block: Bounds, o: Orientation) -> Vec<Point> {
    let (lines, line_len) = if o.by_rows {
        (block.height(), block.width())
    } else {
        (block.width(), block.height())
    };

    (0..lines)
        .flat_map(|k| {
            let line = mirrored(k, lines, o.flip_outer);
            // Every other line is walked backwards; `flip_inner` picks which parity.
            let backwards = (k % 2 == 1) != o.flip_inner;
            (0..line_len).map(move |j| {
                let along = mirrored(j, line_len, backwards);
                if o.by_rows {
                    Point(block.x0 + along, block.y0 + line)
                } else {
                    Point(block.x0 + line, block.y0 + along)
                }
            })
        })
        .filter(|p| pixels.contains(p))
        .collect()
}
