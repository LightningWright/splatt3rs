use std::collections::HashSet;

pub mod grid;
pub mod line;
pub mod tsp_core;
pub mod tsp_optimal;
pub mod tsp_stable;

use crate::{Op, Point};

type Algorithm = fn(HashSet<Point>, AlgorithmSettings) -> Vec<Op>;

#[derive(Clone)]
pub struct AlgorithmSettings {
    pub retries: usize,
    pub workers: usize,
}

/// Execute all algorithms and choose the closest to the optimal path
pub fn run_algorithms(pixels: HashSet<Point>, settings: AlgorithmSettings) -> Vec<Op> {
    let algorithms: &[(&str, Algorithm)] = &[
        ("grid", grid::alg as Algorithm),
        ("line", line::alg as Algorithm),
        ("hpath_stable", tsp_stable::alg as Algorithm),
        ("hpath_tsp", tsp_optimal::alg as Algorithm),
    ];

    let mut best_alg = ("none", Vec::new(), f64::INFINITY);

    for (name, a) in algorithms {
        let res = a(pixels.clone(), settings.clone());

        let instrs = optimize_instr_list(&res);
        // The optimal strategy contains twice the moves/draws than pixels to print
        let opt_instr_ratio = instrs.len() as f64 / pixels.len() as f64;

        println!("[{name}] R {opt_instr_ratio:.05} I {}", res.len());

        if opt_instr_ratio < best_alg.2 {
            best_alg = (name, instrs, opt_instr_ratio);
        }
    }

    println!("Chosen algorithm: {}", best_alg.0);
    println!("Ratio: {:.05}", best_alg.2);
    best_alg.1
}

fn optimize_instr_list(instrs: &[Op]) -> Vec<Op> {
    let mut out = Vec::with_capacity(instrs.len());
    let mut i = 0;

    while i < instrs.len() {
        let cur = instrs[i];

        if !cur.prints() && i + 1 < instrs.len() && instrs[i + 1].eq(&Op::D) {
            out.push(cur.pack());
            i += 2;
        } else {
            out.push(cur);
            i += 1;
        }
    }

    out
}
