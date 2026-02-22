use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use rayon::prelude::*;

use crate::{Op, Point, algorithms::AlgorithmSettings};

pub fn alg(pixels: HashSet<Point>, settings: AlgorithmSettings) -> (Vec<Op>, f64) {
    let best_ratio = Arc::new(Mutex::new(f64::INFINITY));
    let best_instrs = Arc::new(Mutex::new(Vec::new()));

    (0..settings.workers).into_par_iter().for_each(|i| {
        let mut retries = settings.retries;

        while retries != 0 {
            let paths =
                cover_pixels_opportunistic(&pixels, i as i32 - (settings.workers as i32 / 2));
            let instrs = generate_instructions(&paths, &pixels);
            let ratio = instrs.len() as f64 / (pixels.len() as f64);

            // Check and update the best ratio atomically
            let mut locked_best_ratio = best_ratio.lock().unwrap();
            if ratio < *locked_best_ratio {
                // Update best result
                *locked_best_ratio = ratio;
                *best_instrs.lock().unwrap() = instrs;
                retries = settings.retries;
            } else {
                retries -= 1;
            }
        }
    });

    let ratio = *best_ratio.lock().unwrap();
    let instrs = best_instrs.lock().unwrap().clone();

    println!("\r[HSUBPATH-TSP] R {ratio:.5} I {}", instrs.len());

    (instrs, ratio)
}

fn cover_pixels_opportunistic(pixels: &HashSet<Point>, knob: i32) -> Vec<Vec<Point>> {
    let components = connected_components(pixels);
    let graphs: Vec<_> = components.iter().map(build_graph).collect();

    let mut remaining: HashSet<Point> = pixels.clone();
    let mut paths = Vec::new();
    let mut pos = Point(0, 0);

    while !remaining.is_empty() {
        // Start at nearest remaining pixel
        let start = *remaining
            .iter()
            .min_by_key(|p| manhattan(pos, **p))
            .unwrap();

        let comp_idx = graphs.iter().position(|g| g.contains_key(&start)).unwrap();

        let graph = &graphs[comp_idx];
        let mut path = Vec::new();
        let mut current = start;

        path.push(current);
        remaining.remove(&current);

        loop {
            // Best internal continuation
            let next_internal = graph[&current]
                .iter()
                .filter(|p| remaining.contains(p))
                .min_by_key(|p| manhattan(current, **p))
                .copied();

            // Best global alternative
            let next_external = remaining
                .iter()
                .min_by_key(|p| manhattan(current, **p))
                .copied();

            match (next_internal, next_external) {
                (Some(ni), Some(ne)) => {
                    let di = manhattan(current, ni);
                    let de = manhattan(current, ne);

                    if de < di + knob {
                        break;
                    }

                    current = ni;
                }
                (Some(ni), None) => {
                    current = ni;
                }
                _ => break,
            }

            path.push(current);
            remaining.remove(&current);
        }

        pos = current;
        paths.push(path);
    }

    paths
}

fn neighbors(p: Point) -> [Point; 4] {
    [
        Point(p.0 + 1, p.1),
        Point(p.0 - 1, p.1),
        Point(p.0, p.1 + 1),
        Point(p.0, p.1 - 1),
    ]
}

fn build_graph(comp: &HashSet<Point>) -> HashMap<Point, Vec<Point>> {
    let mut g = HashMap::new();
    for &p in comp {
        let mut v = Vec::new();
        for n in neighbors(p) {
            if comp.contains(&n) {
                v.push(n);
            }
        }
        g.insert(p, v);
    }
    g
}

fn connected_components(pixels: &HashSet<Point>) -> Vec<HashSet<Point>> {
    let mut visited = HashSet::new();
    let mut components = Vec::new();

    for &p in pixels {
        if visited.contains(&p) {
            continue;
        }

        let mut stack = vec![p];
        let mut comp = HashSet::new();

        while let Some(q) = stack.pop() {
            if visited.insert(q) {
                comp.insert(q);
                for n in neighbors(q) {
                    if pixels.contains(&n) && !visited.contains(&n) {
                        stack.push(n);
                    }
                }
            }
        }
        components.push(comp);
    }

    components
}

fn manhattan(a: Point, b: Point) -> i32 {
    (a.0 - b.0).abs() + (a.1 - b.1).abs()
}

fn generate_instructions(paths: &[Vec<Point>], image_pixels: &HashSet<Point>) -> Vec<Op> {
    let mut instrs = Vec::new();
    let mut printed = HashSet::new();
    let mut pos = Point(0, 0);

    for path in paths {
        for &p in path {
            let mut dx = p.0 - pos.0;
            let mut dy = p.1 - pos.1;

            if dx != 0 || dy != 0 {
                while dx != 0 {
                    let ddx = (dx / dx.abs()) as i16;
                    let op = if dx > 0 { Op::Xp } else { Op::Xm };

                    instrs.push(op);
                    dx -= ddx as i32;
                }
                while dy != 0 {
                    let ddy = (dy / dy.abs()) as i16;
                    let op = if dy > 0 { Op::Yp } else { Op::Ym };

                    instrs.push(op);
                    dy -= ddy as i32;
                }

                pos = p;
            }

            // Print exactly once per ON pixel
            if image_pixels.contains(&p) && printed.insert(p) {
                instrs.push(Op::D);
            }
        }
    }

    instrs
}
