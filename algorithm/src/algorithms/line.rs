use crate::{
    Op, Point,
    algorithms::{AlgorithmSettings, H, W},
};
use std::collections::HashSet;

pub fn alg(pixels: HashSet<Point>, _: AlgorithmSettings) -> (Vec<Op>, f64) {
    let mut path = Vec::new();
    let mut current = Point(0, 0);

    for x in 0..W {
        // Check if column has any pixels
        let mut has_pixels = false;
        for y in 0..H {
            if pixels.contains(&Point(x, y)) {
                has_pixels = true;
                break;
            }
        }
        if !has_pixels {
            continue;
        }

        let top_down = x % 2 == 0;

        let ys: Box<dyn Iterator<Item = i32>> = if top_down {
            Box::new(0..H)
        } else {
            Box::new((0..H).rev())
        };

        for y in ys {
            let p = Point(x, y);
            if pixels.contains(&p) && current != p {
                path.push(p);
                current = p;
            }
        }
    }

    let instrs = generate_instructions_optimized(&path);
    let ratio = instrs.len() as f64 / pixels.len() as f64;

    println!("\r[LINE] R {ratio:.5} I {}", instrs.len());

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
