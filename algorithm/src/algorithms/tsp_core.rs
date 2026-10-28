//! Shared core for the path-based algorithms.
//!
//! Minimizing the length of the shortest *open path* that visits every pixel, (measured in Manhattan distance) is an
//! open Travelling-Salesman problem (TSP), which this module solves heuristically in four stages:
//!
//! 1. [`Instance::initial_path`]: a greedy "Warnsdorff" walk gives a decent start.
//! 2. [`Tour::run`]: 2-opt and Or-opt local search shortens it until no single local move helps.
//! 3. [`ils`]: iterated local search kicks the tour out of local optima, repairs it, and keeps the result only if it
//!    is not worse.
//! 4. [`build_ops`]: the final visiting order becomes `Xp/Xm/Yp/Ym/D` instructions, painting pixels that are crossed
//!    on the way
//!

use std::{
    cmp::{Ordering, Reverse},
    collections::{HashSet, VecDeque},
    iter,
    ops::Range,
    time::{Duration, Instant},
};

use rand::{Rng, seq::IteratorRandom};

use crate::{Op, Point};

/// Index of a node in an [`Instance`]
pub type Node = u32;

/// Size of each pixel's candidate-neighbour list (see [`Instance::neighbors`]).
///
/// 12 is every cell within Manhattan distance 2 (4 at distance 1 plus 8 at distance 2)
/// Any value from 6 to 24 gives results within about 0.5% of each other, and larger values only cost time
pub const NEIGHBORS: usize = 12;

/// Longest segment (in nodes) that Or-opt tries to relocate.
const MAX_RELOC_SEGMENT: usize = 3;

/// Longest window (in nodes) that a kick swaps. Small, so repairing it is cheap.
const MAX_KICK_WINDOW: usize = 30;

/// Directions: right, left, down, up.
const DIRS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// Manhattan (L1) distance between two points: the number of moves between them.
pub fn manhattan(a: Point, b: Point) -> i32 {
    (a.0 - b.0).abs() + (a.1 - b.1).abs()
}

/// Inclusive rectangle of cells.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl Bounds {
    /// The smallest rectangle containing every point, or `None` if there are none.
    pub fn of(points: impl IntoIterator<Item = Point>) -> Option<Self> {
        let mut points = points.into_iter();
        let first = points.next()?;
        let start = Bounds {
            x0: first.0,
            y0: first.1,
            x1: first.0,
            y1: first.1,
        };
        Some(points.fold(start, Self::including))
    }

    /// Include cell `p` in this rectangle
    fn including(self, p: Point) -> Self {
        Bounds {
            x0: self.x0.min(p.0),
            y0: self.y0.min(p.1),
            x1: self.x1.max(p.0),
            y1: self.y1.max(p.1),
        }
    }

    /// Number of columns.
    pub fn width(&self) -> i32 {
        self.x1 - self.x0 + 1
    }

    /// Number of rows.
    pub fn height(&self) -> i32 {
        self.y1 - self.y0 + 1
    }

    /// Column range
    pub fn xs(&self) -> Range<i32> {
        self.x0..self.x1 + 1
    }

    /// Row range
    pub fn ys(&self) -> Range<i32> {
        self.y0..self.y1 + 1
    }

    /// Whether `p` lies inside the rectangle.
    fn contains(&self, p: Point) -> bool {
        self.xs().contains(&p.0) && self.ys().contains(&p.1)
    }

    /// Row-major index of the cell `p`, or `None` if it is outside.
    fn index_of(&self, p: Point) -> Option<usize> {
        self.contains(p)
            .then(|| ((p.1 - self.y0) * self.width() + (p.0 - self.x0)) as usize)
    }

    /// Number of cells.
    fn area(&self) -> usize {
        (self.width() * self.height()) as usize
    }
}

/// Read-only description of the problem shared by all workers.
pub struct Instance {
    /// Coordinates indexed by node. `coords[0]` is the start, (0,0).
    coords: Vec<Point>,
    /// Row-major grid over `bounds` O(1) access to which pixel is at (x,y)
    grid: Vec<Option<Node>>,
    /// Bounding box of all pixels plus the start; the extent of `grid`.
    bounds: Bounds,
    /// Candidate lists: `nbr[a]` holds up to [`NEIGHBORS`] pixels closest to `a`, nearest first. The local search
    /// only tries moves that connect `a` to one of these, which cuts each step from O(n) to O(K).
    nbr: Vec<Vec<Node>>,
}

impl Instance {
    /// Builds the instance keeping the `k` nearest neighbours of every pixel as move candidates.
    pub fn new(pixels: &HashSet<Point>, k: usize) -> Self {
        let origin = Point(0, 0);
        let bounds = Bounds::of(pixels.iter().copied().chain(iter::once(origin)))
            .expect("the origin is always part of the bounds");

        // Sorted, so node numbering (and therefore every result) is deterministic.
        let mut pixels: Vec<Point> = pixels.iter().copied().collect();
        pixels.sort_unstable_by_key(|p| (p.1, p.0));
        let coords: Vec<Point> = iter::once(origin).chain(pixels).collect();

        let mut grid = vec![None; bounds.area()];
        for (node, &p) in coords.iter().enumerate().skip(1) {
            let cell = bounds.index_of(p).expect("a pixel lies inside the bounds");
            grid[cell] = Some(node as Node);
        }

        let mut instance = Instance {
            coords,
            grid,
            bounds,
            nbr: Vec::new(),
        };

        // Node 0 never takes part in a move, so it gets an empty list.
        instance.nbr = (0..instance.coords.len())
            .map(|node| match node {
                0 => Vec::new(),
                _ => instance.nearest(node as Node, k),
            })
            .collect();
        instance
    }

    /// Coordinates of a node.
    fn coord(&self, node: Node) -> Point {
        self.coords[node as usize]
    }

    /// The node standing on cell `p`, or `None` for an empty or out-of-range cell.
    fn at(&self, p: Point) -> Option<Node> {
        self.bounds.index_of(p).and_then(|cell| self.grid[cell])
    }

    /// The precomputed candidate neighbours of `node`, nearest first.
    pub fn neighbors(&self, node: Node) -> &[Node] {
        &self.nbr[node as usize]
    }

    /// Manhattan distance between two nodes.
    pub fn dist(&self, a: impl Into<Option<Node>>, b: impl Into<Option<Node>>) -> i32 {
        match (a.into(), b.into()) {
            (Some(a), Some(b)) => manhattan(self.coord(a), self.coord(b)),
            _ => 0, // Distance to an open end costs 0
        }
    }

    /// Largest radius worth scanning: beyond it every cell is out of bounds.
    fn max_radius(&self) -> i32 {
        self.bounds.width() + self.bounds.height()
    }

    /// The pixels at Manhattan distance exactly `r` from `center`
    /// Diamond-shaped ring of radius `r` (just `center` itself for `r == 0`).
    fn ring(&self, center: Point, r: i32) -> impl Iterator<Item = Node> + '_ {
        (-r..=r)
            .flat_map(move |dx| {
                let dy = r - dx.abs();
                // At the left and right tips (dy == 0) the two mirrored cells coincide.
                [Some(dy), (dy != 0).then_some(-dy)]
                    .into_iter()
                    .flatten()
                    .map(move |dy| Point(center.0 + dx, center.1 + dy))
            })
            .filter_map(move |p| self.at(p))
    }

    /// The `k` pixels nearest to `node`, found by scanning rings outward.
    fn nearest(&self, node: Node, k: usize) -> Vec<Node> {
        let center = self.coord(node);
        (1..=self.max_radius())
            .flat_map(|r| self.ring(center, r))
            .take(k)
            .collect()
    }

    /// The closest pixel not yet visited, with ties broken uniformly at random.
    fn nearest_unvisited(&self, from: Node, visited: &[bool], rng: &mut impl Rng) -> Option<Node> {
        let center = self.coord(from);
        (0..=self.max_radius()).find_map(|r| self.ring(center, r).filter(|&node| !visited[node as usize]).choose(rng))
    }

    /// The unvisited pixels directly adjacent (4-neighbourhood) to `node`.
    fn free_neighbors<'a>(&'a self, node: Node, visited: &'a [bool]) -> impl Iterator<Item = Node> + Clone + 'a {
        let p = self.coord(node);
        DIRS.into_iter()
            .filter_map(move |(dx, dy)| self.at(Point(p.0 + dx, p.1 + dy)))
            .filter(move |&n| !visited[n as usize])
    }

    /// One step of the Warnsdorff rule: among the adjacent unvisited pixels, pick one with the FEWEST unvisited
    /// neighbours of its own (ties at random). Visiting nearly-stranded pixels first keeps them from being cut off,
    /// so the walk rarely has to jump. `None` means a dead end.
    fn warnsdorff_step(&self, from: Node, visited: &[bool], rng: &mut impl Rng) -> Option<Node> {
        let candidates = self.free_neighbors(from, visited);
        let onward = |n: Node| self.free_neighbors(n, visited).count();
        let fewest = candidates.clone().map(onward).min()?;
        candidates.filter(|&n| onward(n) == fewest).choose(rng)
    }

    /// Greedy starting path, as nodes beginning with the start node 0.
    ///
    /// Walks pixel to adjacent pixel by the Warnsdorff rule; when stuck, jumps to the nearest unvisited pixel and
    /// carries on. Random tie-breaking means different seeds give different starts.
    pub fn initial_path(&self, rng: &mut impl Rng) -> Vec<Node> {
        let mut visited = vec![false; self.coords.len()];
        visited[0] = true;
        let mut path = Vec::with_capacity(self.coords.len());
        path.push(0);

        while path.len() < self.coords.len() {
            let from = path[path.len() - 1];
            let next = self
                .warnsdorff_step(from, &visited, rng)
                .or_else(|| self.nearest_unvisited(from, &visited, rng))
                .expect("an unvisited pixel remains while the path is incomplete");
            visited[next as usize] = true;
            path.push(next);
        }
        path
    }

    /// Converts a node path into the order of pixels to visit (dropping the start).
    pub fn order(&self, path: &[Node]) -> Vec<Point> {
        path.iter().skip(1).map(|&node| self.coord(node)).collect()
    }
}

/// A mutable solution plus the bookkeeping the local search needs.
struct Tour<'a> {
    inst: &'a Instance,
    /// The solution: `path[i]` is the node visited i-th. `path[0]` is always the start node 0.
    path: Vec<Node>,
    /// Inverse of `path`: `pos[node]` is the index of `node` in `path`, so "where is node c?" is O(1).
    /// Every move that shifts or reverses part of `path` must refresh the entries it touched (see [`Tour::reindex`]).
    pos: Vec<u32>,
    /// Nodes whose surroundings changed and may now allow an improving move. Only these are re-examined, so a
    /// local fix costs O(K) instead of rescanning the whole path.
    queue: VecDeque<Node>,
    /// `queued[node]` is true while `node` sits in `queue` (prevents duplicates).
    queued: Vec<bool>,
}

/// Copy of the parts of a [`Tour`] that moves modify; restores the tour after a failed kick without reallocating.
struct TourBackup {
    path: Vec<Node>,
    pos: Vec<u32>,
}

impl<'a> Tour<'a> {
    /// A tour following `path`, with every pixel queued for examination.
    fn new(inst: &'a Instance, path: Vec<Node>) -> Self {
        let queue = path.iter().copied().skip(1).collect();
        let mut queued = vec![true; path.len()];
        queued[0] = false; // the start never moves
        let mut tour = Tour {
            inst,
            pos: vec![0; path.len()],
            path,
            queue,
            queued,
        };
        tour.reindex(0..tour.path.len());
        tour
    }

    /// Total length of the path, i.e. the number of move instructions it needs before `build_ops` saves moves by
    /// painting pixels it crosses.
    fn cost(&self) -> i64 {
        self.path
            .iter()
            .zip(self.path.iter().skip(1))
            .map(|(&a, &b)| i64::from(self.inst.dist(a, b)))
            .sum()
    }

    /// The node at path index `i`, or `None` past the end (the virtual end).
    fn node_at(&self, i: usize) -> Option<Node> {
        self.path.get(i).copied()
    }

    /// Index of `node` in the path.
    fn position(&self, node: Node) -> usize {
        self.pos[node as usize] as usize
    }

    /// Refreshes `pos` for the path indices in `range` after they were changed.
    fn reindex(&mut self, range: Range<usize>) {
        let len = range.len();
        for (i, &node) in self.path.iter().enumerate().skip(range.start).take(len) {
            self.pos[node as usize] = i as u32;
        }
    }

    /// Queues nodes for re-examination.
    fn enqueue(&mut self, nodes: impl IntoIterator<Item = Option<Node>>) {
        for node in nodes.into_iter().flatten().filter(|&n| n != 0) {
            if !std::mem::replace(&mut self.queued[node as usize], true) {
                self.queue.push_back(node);
            }
        }
    }

    /// Reverses the part of the path in `range` and refreshes `pos`.
    fn reverse(&mut self, range: Range<usize>) {
        self.path[range.clone()].reverse();
        self.reindex(range);
    }

    /// Saves the current state into `backup`.
    fn save(&self, backup: &mut TourBackup) {
        backup.path.clone_from(&self.path);
        backup.pos.clone_from(&self.pos);
    }

    /// Restores the state saved in `backup`. The work queue is always empty between kicks.
    fn restore(&mut self, backup: &TourBackup) {
        self.path.clone_from(&backup.path);
        self.pos.clone_from(&backup.pos);
    }

    /// Runs local search until no queued node has an improving move left. Returns the total length saved.
    /// Every applied move strictly shortens the path, so this terminates.
    fn run(&mut self) -> i64 {
        let mut total = 0;
        while let Some(a) = self.queue.pop_front() {
            self.queued[a as usize] = false;
            // Keep improving around `a` until it has no improving move.
            total += iter::from_fn(|| self.improve_node(a)).map(i64::from).sum::<i64>();
        }
        total
    }

    /// Applies the first improving move found around `a` (2-opt first, then Or-opt) and returns its gain, or `None`
    /// if nothing around `a` helps.
    fn improve_node(&mut self, a: Node) -> Option<i32> {
        self.two_opt(a).or_else(|| self.or_opt_around(a))
    }

    /// 2-opt around `a`.
    ///
    /// For each candidate neighbour `c` of `a`, cut two path edges and reconnect them so that `a` and `c` become
    /// adjacent; this reverses the stretch of path between them.
    ///
    /// With `i < j` the positions of `a` and `c`, variants:
    /// -1 cuts the edges after `i` and after `j`, replacing `(i,i+1)` and `(j,j+1)` by `(i,j)` and `(i+1,j+1)`
    /// -2 cuts the edges before `i` and before `j`.
    ///
    /// Because the end is the free virtual node, `j+1` past the end needs no special case.
    /// Returns the gain of the first improving variant applied.
    fn two_opt(&mut self, a: Node) -> Option<i32> {
        let inst = self.inst;
        let pa = self.position(a);
        for &c in inst.neighbors(a) {
            let pc = self.position(c);
            let (i, j) = (pa.min(pc), pa.max(pc));

            let (ni, ni1, nj, nj1) = (self.path[i], self.node_at(i + 1), self.path[j], self.node_at(j + 1));
            let gain = inst.dist(ni, ni1) + inst.dist(nj, nj1) - inst.dist(ni, nj) - inst.dist(ni1, nj1);
            if gain > 0 {
                self.reverse(i + 1..j + 1);
                self.enqueue([Some(ni), ni1, Some(nj), nj1]);
                return Some(gain);
            }

            let (qi, qj) = (self.path[i - 1], self.path[j - 1]);
            let gain = inst.dist(qi, ni) + inst.dist(qj, nj) - inst.dist(qi, qj) - inst.dist(ni, nj);
            if gain > 0 {
                self.reverse(i..j);
                self.enqueue([Some(qi), Some(ni), Some(qj), Some(nj)]);
                return Some(gain);
            }
        }
        None
    }

    /// Or-opt around `a`: tries to relocate a segment of 1 to [`MAX_RELOC_SEGMENT`] nodes that starts or ends at `a`.
    /// Returns the gain of the first improving relocation applied.
    fn or_opt_around(&mut self, a: Node) -> Option<i32> {
        let pa = self.position(a);
        let m = self.path.len();
        (1..=MAX_RELOC_SEGMENT)
            .flat_map(|len| {
                // The segment either starts at `a`, or ends at `a` (so only offered for len > 1).
                let ends_at_a = (len > 1 && pa >= len).then(|| pa + 1 - len);
                iter::once(pa).chain(ends_at_a).map(move |start| (start, len))
            })
            .filter(|&(start, len)| start + len <= m)
            .find_map(|(start, len)| self.or_opt(start, len))
    }

    /// Tries to move the segment `path[start..start + len]` to the best nearby gap, possibly reversed, and applies the
    /// move if it shortens the path.
    ///
    /// A "gap" `t` is the spot between `path[t-1]` and `path[t]`; `t == m` is the very end.
    /// Candidate gaps are the ones right before or after a neighbour of either end of the segment.
    fn or_opt(&mut self, start: usize, len: usize) -> Option<i32> {
        let inst = self.inst;
        let first = self.path[start];
        let last = self.path[start + len - 1];
        let prev = self.path[start - 1];
        let next = self.node_at(start + len);

        // Length saved by cutting the segment out and closing the hole.
        let removal_gain = inst.dist(prev, first) + inst.dist(last, next) - inst.dist(prev, next);
        if removal_gain <= 0 {
            return None; // nothing to gain: inserting anywhere costs >= 0
        }

        let segment = start..start + len;
        let ends = iter::once(first).chain((len > 1).then_some(last));
        let (gain, gap, reversed) = ends
            .flat_map(|end| inst.neighbors(end).iter().copied())
            .map(|c| self.position(c))
            .filter(|pc| !segment.contains(pc))
            .flat_map(|pc| [pc, pc + 1])
            // gap 0 would be before the fixed start; gaps touching the segment are no move
            .filter(|&gap| gap >= 1 && !(start..=start + len).contains(&gap))
            .map(|gap| {
                let left = self.path[gap - 1];
                let right = self.node_at(gap);
                let base = inst.dist(left, right); // the edge we would split
                let forward = inst.dist(left, first) + inst.dist(last, right) - base;
                let backward = inst.dist(left, last) + inst.dist(first, right) - base;
                if forward <= backward {
                    (removal_gain - forward, gap, false)
                } else {
                    (removal_gain - backward, gap, true)
                }
            })
            .filter(|&(gain, ..)| gain > 0)
            // `min_by_key` keeps the first of equal keys, so ties go to the earliest candidate.
            .min_by_key(|&(gain, ..)| Reverse(gain))?;

        let left = self.path[gap - 1];
        let right = self.node_at(gap);

        // Move the segment by rotating the stretch between its old and new place, then flip it in its new place.
        let (changed, placed) = if gap > start + len {
            self.path[start..gap].rotate_left(len);
            (start..gap, gap - len..gap)
        } else {
            self.path[gap..start + len].rotate_right(len);
            (gap..start + len, gap..gap + len)
        };
        if reversed {
            self.path[placed].reverse();
        }
        self.reindex(changed);
        self.enqueue([Some(prev), next, Some(first), Some(last), Some(left), right]);
        Some(gain)
    }

    /// The kick of iterated local search: swaps two adjacent windows A and B of 1 to [`MAX_KICK_WINDOW`] nodes each
    /// (A B becomes B A), a local "double bridge". It is local so that repairing it is cheap.
    /// Returns the change in path length (usually positive, i.e. worse); the caller repairs it with [`Tour::run`].
    fn perturb(&mut self, rng: &mut impl Rng) -> i64 {
        let inst = self.inst;
        let m = self.path.len();
        let max_len = MAX_KICK_WINDOW.min((m - 1) / 2);
        let len_a = rng.random_range(1..=max_len);
        let len_b = rng.random_range(1..=max_len);
        let i = rng.random_range(1..=m - len_a - len_b);
        let j = i + len_a;
        let k = j + len_b;

        let (a0, a1) = (self.path[i - 1], self.path[i]);
        let (b0, b1) = (self.path[j - 1], self.path[j]);
        let (c0, c1) = (self.path[k - 1], self.node_at(k));
        let delta = inst.dist(a0, b1) + inst.dist(c0, a1) + inst.dist(b0, c1)
            - inst.dist(a0, a1)
            - inst.dist(b0, b1)
            - inst.dist(c0, c1);

        self.path[i..k].rotate_left(len_a);
        self.reindex(i..k);
        self.enqueue([Some(a0), Some(a1), Some(b0), Some(b1), Some(c0), c1]);
        i64::from(delta)
    }
}

/// A finished tour and its length.
pub struct Solution {
    /// Node path beginning with the start node (see [`Instance::order`]).
    pub path: Vec<Node>,
    /// Path length, i.e. the number of moves before painting-while-crossing savings.
    pub cost: i64,
}

/// Iterated local search.
///
/// 1. Build a greedy start path and run local search to a first local optimum.
/// 2. Repeat: snapshot the tour, kick it ([`Tour::perturb`]), repair it with local search, and keep the result if
///    the path did not get longer; otherwise restore the snapshot.
///
/// Stops after `max_fails` consecutive kicks that did not shorten the path, or when nothing has improved for `stall`.
/// `max_fails == 0` skips the kicks and returns the plain local-search result.
pub fn ils(inst: &Instance, rng: &mut impl Rng, max_fails: usize, stall: Duration) -> Solution {
    let mut tour = Tour::new(inst, inst.initial_path(rng));
    let mut cost = tour.cost() - tour.run();

    // Snapshot buffers, reused every iteration to avoid allocating.
    let mut backup = TourBackup {
        path: tour.path.clone(),
        pos: tour.pos.clone(),
    };
    let mut fails = 0;
    let mut last_improvement = Instant::now();

    while tour.path.len() >= 4 && fails < max_fails && last_improvement.elapsed() < stall {
        tour.save(&mut backup);
        let delta = tour.perturb(rng) - tour.run(); // net change after repair
        match delta.cmp(&0) {
            Ordering::Less => {
                cost += delta;
                fails = 0;
                last_improvement = Instant::now();
            }
            // A sideways move is kept (it lets the search drift) but counts as a failure.
            Ordering::Equal => fails += 1,
            Ordering::Greater => {
                tour.restore(&backup);
                fails += 1;
            }
        }
    }
    Solution { path: tour.path, cost }
}

/// Generate an iterator of steps between a and b.
fn walk(a: i32, b: i32) -> impl Iterator<Item = i32> {
    let delta = b - a;
    (1..=delta.abs()).map(move |k| a + k * delta.signum())
}

/// Generate a route (L-shaped), from `from` Point (excluding it) to `to` (including it). Traversal is horizontal
/// first if `x_first` is set, then vertical, or viceversa if `x_first` is false.
///
/// I.e: x_first = true -> (2,1), (3,1), (3,2), (3,3); x_first = false -> (1,2), (1,3), (2,3), (3,3)
fn route(from: Point, to: Point, x_first: bool) -> Vec<Point> {
    if x_first {
        walk(from.0, to.0)
            .map(|x| Point(x, from.1))
            .chain(walk(from.1, to.1).map(|y| Point(to.0, y)))
            .collect()
    } else {
        walk(from.1, to.1)
            .map(|y| Point(from.0, y))
            .chain(walk(from.0, to.0).map(|x| Point(x, to.1)))
            .collect()
    }
}

/// The instruction that moves the cursor from `from` to the adjacent cell `to`.
fn step_op(from: Point, to: Point) -> Op {
    match (to.0 - from.0, to.1 - from.1) {
        (1, 0) => Op::Xp,
        (-1, 0) => Op::Xm,
        (0, 1) => Op::Yp,
        (0, -1) => Op::Ym,
        step => unreachable!("not a unit step: {step:?}"),
    }
}

/// Turns a visiting order into instructions.
///
/// Between two options it picks the L-shaped route that crosses more still-unpainted pixels, paints every unpainted
/// pixel it crosses on the spot (free, since the cursor passes over it anyway), and skips later visits to pixels that
/// are already painted.
pub fn build_ops(pixels: &HashSet<Point>, order: &[Point]) -> Vec<Op> {
    let origin = Point(0, 0);
    let mut ops = Vec::with_capacity(pixels.len() * 2);
    let mut painted: HashSet<Point> = HashSet::with_capacity(pixels.len());
    let mut cursor = origin;
    if pixels.contains(&origin) {
        painted.insert(origin);
        ops.push(Op::D);
    }

    for &target in order {
        if painted.contains(&target) || !pixels.contains(&target) {
            continue;
        }
        let unpainted = |route: &[Point]| {
            route
                .iter()
                .filter(|cell| pixels.contains(cell) && !painted.contains(cell))
                .count()
        };
        let horizontal_first = route(cursor, target, true);
        let vertical_first = route(cursor, target, false);
        let chosen = if unpainted(&horizontal_first) >= unpainted(&vertical_first) {
            horizontal_first
        } else {
            vertical_first
        };

        for cell in chosen {
            ops.push(step_op(cursor, cell));
            cursor = cell;
            if pixels.contains(&cell) && painted.insert(cell) {
                ops.push(Op::D);
            }
        }
    }
    ops
}
