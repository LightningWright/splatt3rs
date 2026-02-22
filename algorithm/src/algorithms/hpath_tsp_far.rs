use std::{
    collections::{HashMap, HashSet},
    io::Write,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use rayon::prelude::*;

use crate::{Op, Point, algorithms::AlgorithmSettings};

pub fn alg(pixels: HashSet<Point>, settings: AlgorithmSettings) -> (Vec<Op>, f64) {
    let best_ratio = Arc::new(Mutex::new(f64::INFINITY));
    let best_instrs = Arc::new(Mutex::new(Vec::new()));

    (0..settings.workers).into_par_iter().for_each(|_| {
        let mut best_find = Instant::now();
        let mut timeout = false;
        let mut retries = settings.retries;

        while retries != 0 && !timeout {
            let components = connected_components(&pixels);
            let mut all_paths = Vec::new();

            for comp in &components {
                let graph = build_graph(comp);
                let path = cover_all_pixels(&graph);
                all_paths.push(path);
            }

            let ordered = order_components(&all_paths);
            let instrs = generate_instructions(&ordered, &pixels);

            let ratio = instrs.len() as f64 / (pixels.len() as f64);

            // Check and update the best ratio atomically
            let mut locked_best_ratio = best_ratio.lock().unwrap();
            if ratio < *locked_best_ratio {
                // Update best result
                *locked_best_ratio = ratio;
                print!(
                    "\r[HPATH-TSP-FAR] R {locked_best_ratio:.5} I {}",
                    instrs.len()
                );
                *best_instrs.lock().unwrap() = instrs;
                retries = settings.retries;
                best_find = Instant::now();
            } else {
                if best_find.elapsed() > Duration::from_secs(10) {
                    timeout = true;
                }
                retries -= 1;
            }
            std::io::stdout().flush().unwrap();
        }
    });

    let ratio = *best_ratio.lock().unwrap();
    let instrs = best_instrs.lock().unwrap().clone();

    println!("\r[HPATH-TSP-FAR] R {ratio:.5} I {}", instrs.len());

    (instrs, ratio)
}

fn neighbors(p: Point) -> [Point; 4] {
    [
        Point(p.0 + 1, p.1),
        Point(p.0 - 1, p.1),
        Point(p.0, p.1 + 1),
        Point(p.0, p.1 - 1),
    ]
}

fn neighbors_far(p: Point) -> [Point; 4] {
    [
        Point(p.0 + 1, p.1 + 1), //
        Point(p.0 + 1, p.1 - 1), //
        Point(p.0 - 1, p.1 + 1), //
        Point(p.0 - 1, p.1 - 1), //
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
                let mut nb_visited = false;
                for n in neighbors(q) {
                    if pixels.contains(&n) && !visited.contains(&n) {
                        stack.push(n);
                        nb_visited = true;
                    }
                }
                if !nb_visited {
                    for n in neighbors_far(q) {
                        if pixels.contains(&n) && !visited.contains(&n) {
                            stack.push(n);
                        }
                    }
                }
            }
        }
        components.push(comp);
    }

    components
}

fn cover_all_pixels(graph: &HashMap<Point, Vec<Point>>) -> Vec<Point> {
    let mut visited = HashSet::new();
    let mut path = Vec::new();

    let mut current = graph
        .iter()
        .find(|(_, v)| v.len() == 1)
        .map(|(&k, _)| k)
        .unwrap_or_else(|| *graph.keys().next().unwrap());

    path.push(current);
    visited.insert(current);

    while visited.len() < graph.len() {
        // Find unvisited neighbors
        let unvisited_neighbors: Vec<_> = graph[&current]
            .iter()
            .copied()
            .filter(|p| !visited.contains(p))
            .collect();

        if !unvisited_neighbors.is_empty() {
            // Move to the neighbor with fewest unvisited neighbors (forced moves first)
            let &next = unvisited_neighbors
                .iter()
                .min_by_key(|p| graph[p].iter().filter(|n| !visited.contains(n)).count())
                .unwrap();

            path.push(next);
            visited.insert(next);
            current = next;
        } else {
            // No unvisited neighbors, jump to the closest unvisited pixel in the component
            let &next = graph
                .keys()
                .filter(|p| !visited.contains(p))
                .min_by_key(|p| manhattan(current, **p))
                .unwrap();
            path.push(next);
            visited.insert(next);
            current = next;
        }
    }

    path
}

fn manhattan(a: Point, b: Point) -> i32 {
    (a.0 - b.0).abs() + (a.1 - b.1).abs()
}

fn order_components(paths: &[Vec<Point>]) -> Vec<Vec<Point>> {
    let mut remaining: Vec<Vec<Point>> = paths.to_vec();
    let mut ordered = Vec::new();
    let mut current = Point(0, 0);

    while !remaining.is_empty() {
        let (idx, reverse) = remaining
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let d_start = manhattan(current, p[0]);
                let d_end = manhattan(current, *p.last().unwrap());

                if d_start <= d_end {
                    (i, false, d_start)
                } else {
                    (i, true, d_end)
                }
            })
            .min_by_key(|(_, _, d)| *d)
            .map(|(i, r, _)| (i, r))
            .unwrap();

        let mut path = remaining.remove(idx);

        if reverse {
            path.reverse();
        }

        current = *path.last().unwrap();
        ordered.push(path);
    }

    ordered
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
