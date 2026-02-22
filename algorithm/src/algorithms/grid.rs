use rayon::prelude::*;
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use crate::{
    Op, Point,
    algorithms::{AlgorithmSettings, H, W},
};

pub fn alg(pixels: HashSet<Point>, _: AlgorithmSettings) -> (Vec<Op>, f64) {
    let best_ratio = Arc::new(Mutex::new(f64::INFINITY));
    let best_instrs = Arc::new(Mutex::new(Vec::new()));

    let block_sizes = [2, 4, 5, 8, 10, 20, 40, 120];

    block_sizes.into_par_iter().for_each(|block_sz| {
        let mut path = Vec::new();
        let mut current = Point(0, 0);

        // Rows of blocks
        let block_rows = (0..H).step_by(block_sz as usize).collect::<Vec<_>>();

        for (row_idx, by) in block_rows.iter().enumerate() {
            // Determine horizontal block order for serpentine
            let mut block_cols = (0..W).step_by(block_sz as usize).collect::<Vec<_>>();
            if row_idx % 2 == 1 {
                block_cols.reverse();
            }

            for &bx in &block_cols {
                // Check if the 8x8 block has any pixels
                let mut has_pixels = false;
                'check_block: for y in *by..(*by + block_sz).min(H) {
                    for x in bx..(bx + block_sz).min(W) {
                        if pixels.contains(&Point(x, y)) {
                            has_pixels = true;
                            break 'check_block;
                        }
                    }
                }
                if !has_pixels {
                    continue; // skip empty block
                }

                // Serpentine scan inside the block
                for (inner_row_idx, y) in (*by..(*by + block_sz).min(H)).enumerate() {
                    let ltr = inner_row_idx % 2 == 0; // left→right on even row
                    let xs: Box<dyn Iterator<Item = i32>> = if ltr {
                        Box::new(bx..(bx + block_sz).min(W))
                    } else {
                        Box::new(((bx)..(bx + block_sz).min(W)).rev())
                    };

                    for x in xs {
                        let p = Point(x, y);
                        if pixels.contains(&p) && current != p {
                            path.push(p);
                            current = p;
                        }
                    }
                }
            }
        }

        let instrs = generate_instructions_optimized(&path);
        let ratio = instrs.len() as f64 / pixels.len() as f64;

        let mut locked_best_ratio = best_ratio.lock().unwrap();
        if ratio < *locked_best_ratio {
            *locked_best_ratio = ratio;
            let mut locked_best_instrs = best_instrs.lock().unwrap();
            *locked_best_instrs = instrs;
            print!("\rNew ratio {} @ size {}", locked_best_ratio, block_sz);
        }
    });

    let ratio = *best_ratio.lock().unwrap();
    let instrs = best_instrs.lock().unwrap().clone();

    println!("\r[GRID] R {ratio:.5} I {}               ", instrs.len());

    (instrs, ratio)
}

fn generate_instructions_optimized(path: &[Point]) -> Vec<Op> {
    let mut instrs = Vec::with_capacity(path.len() * 2);
    let mut pos = Point(0, 0);

    for &p in path {
        let mut dx = p.0 - pos.0;
        let mut dy = p.1 - pos.1;

        while dx != 0 {
            let ddx = dx.signum();
            instrs.push(if ddx > 0 { Op::Xp } else { Op::Xm });
            dx -= ddx;
        }

        while dy != 0 {
            let ddy = dy.signum();
            instrs.push(if ddy > 0 { Op::Yp } else { Op::Ym });
            dy -= ddy;
        }

        instrs.push(Op::D);
        pos = p;
    }

    instrs
}
