use std::collections::{HashMap, HashSet};

use crate::{Op, Point, algorithms::AlgorithmSettings};

pub fn alg(pixels: HashSet<Point>, _: AlgorithmSettings) -> (Vec<Op>, f64) {
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

    println!("\r[STABLE] R {ratio:.5} I {}               ", instrs.len());
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

fn manhattan(a: Point, b: Point) -> i32 {
    (a.0 - b.0).abs() + (a.1 - b.1).abs()
}

// Connected components using DFS
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
                    let ddx = dx.signum();
                    instrs.push(if ddx > 0 { Op::Xp } else { Op::Xm });
                    dx -= ddx;
                }
                while dy != 0 {
                    let ddy = dy.signum();
                    instrs.push(if ddy > 0 { Op::Yp } else { Op::Ym });
                    dy -= ddy;
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
