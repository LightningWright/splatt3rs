//! Deterministic, fast TSP-based algorithm.
//!
//! Visits every pixel once along a short path, found in two cheap steps:
//!
//! 1. A greedy Warnsdorff walk (always step to the adjacent unvisited pixel with the fewest unvisited neighbours,
//!    jump to the nearest pixel when stuck)
//! 2. A single round of 2-opt / Or-opt local search that shortens it until no local move helps.

use std::{collections::HashSet, time::Duration};

use rand::{SeedableRng, rngs::SmallRng};

use crate::{
    Op, Point,
    algorithms::{
        AlgorithmSettings,
        tsp_core::{Instance, NEIGHBORS, build_ops, ils},
    },
};

/// Fixed seed, so the "random" tie-breaking is reproducible run to run.
const SEED: u64 = 0xC0FFEE;

/// Runs one greedy walk plus local search and returns the instructions
pub fn alg(pixels: HashSet<Point>, _: AlgorithmSettings) -> Vec<Op> {
    if pixels.is_empty() {
        return Vec::new();
    }
    let inst = Instance::new(&pixels, NEIGHBORS);
    let mut rng = SmallRng::seed_from_u64(SEED);
    // `max_fails == 0`: no kicks, just the local-search optimum.
    let solution = ils(&inst, &mut rng, 0, Duration::ZERO);

    build_ops(&pixels, &inst.order(&solution.path))
}
