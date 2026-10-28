//! Quasi-Optimal TSP-based algorithm.
//!
//! Solve the open Travelling-Salesman problem running *ILS* on several independent workers in parallel:
//!
//! 1. Each worker builds its own randomized greedy start path and shortens it with 2-opt / Or-opt local search
//! 2. Then, repeatedly kicks the tour (swaps two small adjacent windows)
//! 3. Repairs it with local search and keeps the result if it is not longer
//! 4. Stops after `settings.retries` kicks in a row that did not help, or after `STALL` seconds without an improvement
//! 5. The shortest tour over all workers wins.
//!
//! Different seeds make the workers explore different parts of the search space.

use std::{collections::HashSet, time::Duration};

use rand::{SeedableRng, rngs::SmallRng};
use rayon::prelude::*;

use crate::{
    Op, Point,
    algorithms::{
        AlgorithmSettings,
        tsp_core::{Instance, NEIGHBORS, build_ops, ils},
    },
};

/// Base seed; worker `w` uses `SEED + w`, so runs are reproducible.
const SEED: u64 = 0xC0FFEE;

/// How long a worker may go without improving before it gives up.
const STALL: Duration = Duration::from_secs(10);

/// Runs `settings.workers` iterated-local-search workers in parallel and
/// returns the instructions of the best tour with their ratio.
pub fn alg(pixels: HashSet<Point>, settings: AlgorithmSettings) -> Vec<Op> {
    if pixels.is_empty() {
        return Vec::new();
    }
    let inst = Instance::new(&pixels, NEIGHBORS);
    let max_fails = settings.retries;

    let best = (0..settings.workers.max(1))
        .into_par_iter()
        .map(|worker| {
            let mut rng = SmallRng::seed_from_u64(SEED + worker as u64);
            ils(&inst, &mut rng, max_fails, STALL)
        })
        .min_by_key(|solution| solution.cost)
        .expect("there is at least one worker");

    build_ops(&pixels, &inst.order(&best.path))
}
