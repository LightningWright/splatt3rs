//! Column/row sweep: fast to compute, but far from optimal.
//!
//! Group the pixels into lines (all columns, or all rows), visit the lines in
//! order, and walk each line in whichever direction starts closest to the
//! cursor, so consecutive lines are joined like a snake (boustrophedon). The
//! cursor never goes back, but empty stretches inside a line are walked
//! across, which is why sparse images do badly. Both orientations are tried
//! and the shorter result is kept.

use std::collections::HashSet;

use crate::{
    Op, Point,
    algorithms::{
        AlgorithmSettings,
        tsp_core::{build_ops, manhattan},
    },
};

/// Which way the lines of a sweep run.
#[derive(Clone, Copy)]
enum Axis {
    /// Vertical lines, visited left to right.
    Columns,
    /// Horizontal lines, visited top to bottom.
    Rows,
}

impl Axis {
    /// Which line the pixel is on (the x coordinate for columns, y for rows).
    fn line_of(self, p: Point) -> i32 {
        match self {
            Axis::Columns => p.0,
            Axis::Rows => p.1,
        }
    }

    /// Where the pixel is along its line.
    fn along(self, p: Point) -> i32 {
        match self {
            Axis::Columns => p.1,
            Axis::Rows => p.0,
        }
    }
}

/// Sweeps by columns and by rows and returns the instructions of the shorter
/// one, with their ratio.
pub fn alg(pixels: HashSet<Point>, _: AlgorithmSettings) -> Vec<Op> {
    if pixels.is_empty() {
        return Vec::new();
    }
    [Axis::Columns, Axis::Rows]
        .into_iter()
        .map(|axis| build_ops(&pixels, &sweep(&pixels, axis)))
        .min_by_key(|ops| ops.len())
        .expect("two sweeps were tried")
}

/// The visiting order of one sweep: lines in order, each line walked in the
/// direction whose first pixel is closer to where the cursor currently is.
fn sweep(pixels: &HashSet<Point>, axis: Axis) -> Vec<Point> {
    let mut pts: Vec<Point> = pixels.iter().copied().collect();
    pts.sort_unstable_by_key(|&p| (axis.line_of(p), axis.along(p)));

    let mut order = Vec::with_capacity(pts.len());
    let mut cursor = Point(0, 0);
    // After sorting, the pixels of one line are adjacent: split into lines.
    for line in pts.chunk_by(|a, b| axis.line_of(*a) == axis.line_of(*b)) {
        let (Some(&first), Some(&last)) = (line.first(), line.last()) else {
            continue;
        };
        if manhattan(cursor, first) <= manhattan(cursor, last) {
            order.extend(line.iter().copied());
            cursor = last;
        } else {
            order.extend(line.iter().rev().copied());
            cursor = first;
        }
    }
    order
}
