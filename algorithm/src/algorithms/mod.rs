use std::collections::HashSet;

pub mod grid;
pub mod hpath_stable;
pub mod hpath_tsp_far;
pub mod hpath_tsp_near;
pub mod hsubpath_tsp;
pub mod line;

use crate::{Op, Point};

const W: i32 = 320;
const H: i32 = 120;

type Algorithm = fn(HashSet<Point>, AlgorithmSettings) -> (Vec<Op>, f64);

#[derive(Clone)]
pub struct AlgorithmSettings {
    pub retries: usize,
    pub workers: usize,
}

pub fn run_algorithms(pixels: HashSet<Point>, settings: AlgorithmSettings) -> Vec<Op> {
    let algorithms: &[(&str, Algorithm)] = &[
        ("stable", hpath_stable::alg as Algorithm),
        ("grid", grid::alg as Algorithm),
        ("line", line::alg as Algorithm),
        ("hpath_tsp_far", hpath_tsp_far::alg as Algorithm),
        ("hpath_tsp_near", hpath_tsp_near::alg as Algorithm),
        ("hsubpath_tsp", hsubpath_tsp::alg as Algorithm),
    ];

    let mut best_alg = ("none", (Vec::new(), f64::INFINITY));

    for (name, a) in algorithms {
        print!("Executing {name}");
        let res = a(pixels.clone(), settings.clone());

        if res.1 < best_alg.1.1 {
            best_alg = (name, res);
        }
    }

    println!("Chosen algorithm: {}", best_alg.0);
    println!("Ratio: {}", best_alg.1.1);
    best_alg.1.0
}
