//! Bentley–Ottmann sweep that splits flattened segments at every crossing.
//!
//! The intersection stage of the planar map (`region::build`) used to test segment pairs inside
//! a uniform grid. This sweep reports the same split parameters [`crate::line_line`] would, in
//! O((n + k) log n) event-queue operations, where k is the number of crossings.
//!
//! # Intersection coordinates
//!
//! A crossing is stored as the rounded `f64` point from [`crate::line_line_infinite`]: the
//! parameters are ratios of exact [`crate::predicates::orient2d`] determinants, and the division
//! is the rounding. Split parameters are then [`crate::Line::param_of`] of that point, clamped
//! to `[0, 1]`, which is what the grid stage wrote.
//!
//! Exact rationals are not carried. An intersection is a degree-3 rational, but the graph the
//! face walk sees is already `f64` vertices merged into tolerance-sized bins (`region`'s `key`).
//! Crossing versus missing is decided by the exact predicate [`crate::segment_intersects`], so a
//! few ulps of error in the point do not invent or drop a crossing. Two rounded images of the
//! same crossing fall in one bin whenever the error is below half the caller's tolerance, which
//! holds for coordinates inside the predicate's exponent range at CAD tolerances (`1e-9` and
//! coarser). The walk then uses [`crate::predicates::orient2d`] only to pick which side of the
//! hit edge the seed lies on; the next edge is chosen from vertices that have already been merged.
//!
//! # Degeneracies
//!
//! * **Vertical segments** are not stored in the y-ordered status (they are not a function of x).
//!   At their x, every active segment whose line separates the vertical's endpoints, or meets it
//!   under [`crate::segment_intersects`], is split with the same `line_line` point.
//! * **Overlapping collinear segments** have no single crossing (`line_line` is `None`, matching
//!   the grid). They stay tied in the status and are ordered by segment id. A transversal is
//!   tested against the whole tied run, not only the id-minimum.
//! * **Several segments through one point** are one batch: every event with that exact `f64`
//!   point is popped together, those segments are removed, and the ones that continue to the
//!   right are reinserted in outgoing-angle order ([`orient2d`](crate::orient2d) on direction
//!   vectors).

use std::cmp::{Ordering, Reverse};
use std::collections::{BinaryHeap, HashMap, HashSet};

use crate::predicates::{Orientation, orient2d, point_on_segment, segment_intersects};
use crate::{Line, Vec2, line_line};

/// Hostile-input bound on crossings materialized by the sweep.
///
/// This caps output size, not candidate pairs. A drawing under [`crate::region::MAX_SEGMENTS`]
/// finishes when it has fewer crossings than this. Random segments that span a square cross
/// about a quarter of the time, so a few tens of thousands of them still hit the cap; the
/// segment cap alone does not bound that quadratic output.
const MAX_CROSSINGS: usize = 80_000_000;

const NIL: u32 = u32::MAX;

#[derive(Clone, Copy)]
struct Seg {
    a: Vec2,
    b: Vec2,
    /// Lexicographically smaller endpoint.
    left: Vec2,
    /// Lexicographically larger endpoint.
    right: Vec2,
    vertical: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Visit this x so a vertical segment is tested. Not a status edit.
    Column = 0,
    End = 1,
    Cross = 2,
    Start = 3,
}

#[derive(Clone, Copy)]
struct Ev {
    x: u64,
    y: u64,
    kind: Kind,
    a: u32,
    b: u32,
}

impl PartialEq for Ev {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for Ev {}
impl PartialOrd for Ev {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Ev {
    fn cmp(&self, o: &Self) -> Ordering {
        self.x.cmp(&o.x).then(self.y.cmp(&o.y)).then((self.kind as u8).cmp(&(o.kind as u8))).then(self.a.cmp(&o.a)).then(self.b.cmp(&o.b))
    }
}

fn lex_cmp(a: Vec2, b: Vec2) -> Ordering {
    a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y))
}

/// Map `total_cmp` onto `u64` so the event heap orders finite coordinates exactly.
fn total_key(v: f64) -> u64 {
    let bits = v.to_bits();
    if bits & 0x8000_0000_0000_0000 == 0 { bits | 0x8000_0000_0000_0000 } else { !bits }
}

fn from_total_key(k: u64) -> f64 {
    let bits = if k & 0x8000_0000_0000_0000 == 0 { !k } else { k & !0x8000_0000_0000_0000 };
    f64::from_bits(bits)
}

fn ev_point(e: Ev) -> Vec2 {
    Vec2::new(from_total_key(e.x), from_total_key(e.y))
}

struct Node {
    seg: u32,
    pri: u32,
    left: u32,
    right: u32,
    parent: u32,
}

/// Max-heap treap. Search compares only the inserted segment against nodes already in the tree.
struct Treap {
    n: Vec<Node>,
    free: Vec<u32>,
    root: u32,
    pos: Vec<u32>,
    rng: u32,
}

impl Treap {
    fn new(segs: usize) -> Self {
        Self { n: Vec::new(), free: Vec::new(), root: NIL, pos: vec![NIL; segs], rng: 0xA5A5_1234 }
    }

    fn contains(&self, seg: u32) -> bool {
        self.pos.get(seg as usize).is_some_and(|&p| p != NIL)
    }

    fn node(&self, i: u32) -> Option<&Node> {
        self.n.get(i as usize)
    }

    fn pri_of(&mut self) -> u32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        if x == 0 {
            x = 1;
        }
        self.rng = x;
        x
    }

    fn alloc(&mut self, seg: u32, pri: u32) -> u32 {
        let node = Node { seg, pri, left: NIL, right: NIL, parent: NIL };
        if let Some(i) = self.free.pop() {
            if let Some(slot) = self.n.get_mut(i as usize) {
                *slot = node;
            }
            i
        } else {
            self.n.push(node);
            (self.n.len() - 1) as u32
        }
    }

    fn child(&self, i: u32, right: bool) -> u32 {
        self.node(i).map(|n| if right { n.right } else { n.left }).unwrap_or(NIL)
    }

    fn set_child(&mut self, i: u32, right: bool, v: u32) {
        if let Some(n) = self.n.get_mut(i as usize) {
            if right { n.right = v } else { n.left = v }
        }
    }

    fn parent(&self, i: u32) -> u32 {
        self.node(i).map(|n| n.parent).unwrap_or(NIL)
    }

    fn set_parent(&mut self, i: u32, v: u32) {
        if let Some(n) = self.n.get_mut(i as usize) {
            n.parent = v;
        }
    }

    fn seg_of(&self, i: u32) -> u32 {
        self.node(i).map(|n| n.seg).unwrap_or(NIL)
    }

    fn pri(&self, i: u32) -> u32 {
        self.node(i).map(|n| n.pri).unwrap_or(0)
    }

    fn rotate_right(&mut self, x: u32) {
        let y = self.child(x, false);
        if y == NIL {
            return;
        }
        let b = self.child(y, true);
        let p = self.parent(x);
        self.set_child(y, true, x);
        self.set_parent(x, y);
        self.set_child(x, false, b);
        if b != NIL {
            self.set_parent(b, x);
        }
        self.set_parent(y, p);
        self.attach(p, x, y);
    }

    fn rotate_left(&mut self, x: u32) {
        let y = self.child(x, true);
        if y == NIL {
            return;
        }
        let b = self.child(y, false);
        let p = self.parent(x);
        self.set_child(y, false, x);
        self.set_parent(x, y);
        self.set_child(x, true, b);
        if b != NIL {
            self.set_parent(b, x);
        }
        self.set_parent(y, p);
        self.attach(p, x, y);
    }

    fn attach(&mut self, p: u32, was: u32, now: u32) {
        if p == NIL {
            self.root = now;
        } else if self.child(p, false) == was {
            self.set_child(p, false, now);
        } else {
            self.set_child(p, true, now);
        }
    }

    fn insert(&mut self, seg: u32, cmp: &mut dyn FnMut(u32, u32) -> Ordering) {
        if self.contains(seg) {
            return;
        }
        let pri = self.pri_of();
        let idx = self.alloc(seg, pri);
        if let Some(p) = self.pos.get_mut(seg as usize) {
            *p = idx;
        }
        if self.root == NIL {
            self.root = idx;
            return;
        }
        let mut cur = self.root;
        let mut guard = 0;
        while guard < self.n.len().saturating_add(1) {
            guard += 1;
            let cseg = self.seg_of(cur);
            let go_right = cmp(seg, cseg) != Ordering::Less;
            let nxt = self.child(cur, go_right);
            if nxt == NIL {
                self.set_child(cur, go_right, idx);
                self.set_parent(idx, cur);
                break;
            }
            cur = nxt;
        }
        self.bubble(idx);
    }

    fn bubble(&mut self, idx: u32) {
        let mut guard = 0;
        while guard < self.n.len().saturating_add(1) {
            guard += 1;
            let p = self.parent(idx);
            if p == NIL || self.pri(p) >= self.pri(idx) {
                break;
            }
            if self.child(p, false) == idx {
                self.rotate_right(p);
            } else {
                self.rotate_left(p);
            }
        }
    }

    fn remove(&mut self, seg: u32) {
        let Some(&mut_idx) = self.pos.get(seg as usize) else { return };
        if mut_idx == NIL {
            return;
        }
        let x = mut_idx;
        if let Some(p) = self.pos.get_mut(seg as usize) {
            *p = NIL;
        }
        let mut guard = 0;
        let limit = self.n.len().saturating_add(1);
        while guard < limit {
            guard += 1;
            let l = self.child(x, false);
            let r = self.child(x, true);
            if l == NIL && r == NIL {
                let p = self.parent(x);
                if p == NIL {
                    self.root = NIL;
                } else if self.child(p, false) == x {
                    self.set_child(p, false, NIL);
                } else {
                    self.set_child(p, true, NIL);
                }
                self.free.push(x);
                return;
            }
            let left_up = r == NIL || (l != NIL && self.pri(l) > self.pri(r));
            if left_up {
                self.rotate_right(x);
            } else {
                self.rotate_left(x);
            }
        }
    }

    fn neighbor(&self, seg: u32, succ: bool) -> Option<u32> {
        let x = self.pos.get(seg as usize).copied().unwrap_or(NIL);
        if x == NIL {
            return None;
        }
        let straight = self.child(x, succ);
        if straight != NIL {
            let mut c = straight;
            let mut guard = 0;
            while self.child(c, !succ) != NIL && guard < self.n.len() {
                c = self.child(c, !succ);
                guard += 1;
            }
            let s = self.seg_of(c);
            return (s != NIL).then_some(s);
        }
        let mut c = x;
        let mut p = self.parent(c);
        let mut guard = 0;
        while p != NIL && self.child(p, succ) == c && guard < self.n.len() {
            c = p;
            p = self.parent(c);
            guard += 1;
        }
        if p == NIL { None } else { Some(self.seg_of(p)) }
    }

    /// First in-order segment whose line is not strictly below `p`.
    fn first_not_below(&self, segs: &[Seg], p: Vec2) -> Option<u32> {
        let mut cur = self.root;
        let mut cand = NIL;
        let mut guard = 0;
        while cur != NIL && guard < self.n.len().saturating_add(1) {
            guard += 1;
            let id = self.seg_of(cur);
            if segs.get(id as usize).is_some_and(|s| point_vs_seg(s, p) == Ordering::Greater) {
                cur = self.child(cur, true);
            } else {
                cand = cur;
                cur = self.child(cur, false);
            }
        }
        (cand != NIL).then(|| self.seg_of(cand))
    }
}

/// `Greater` when `p` is strictly above the left-to-right segment.
fn point_vs_seg(seg: &Seg, p: Vec2) -> Ordering {
    match orient2d(seg.left, seg.right, p) {
        Orientation::Ccw => Ordering::Greater,
        Orientation::Cw => Ordering::Less,
        Orientation::Collinear => Ordering::Equal,
    }
}

/// `Less` when `a` leaves `p` below `b` (counter-clockwise is above, just to the right of `p`).
fn slope_order(a: &Seg, b: &Seg) -> Ordering {
    match orient2d(Vec2::ZERO, a.right - a.left, b.right - b.left) {
        Orientation::Ccw => Ordering::Less,
        Orientation::Cw => Ordering::Greater,
        Orientation::Collinear => Ordering::Equal,
    }
}

fn cmp_ids(segs: &[Seg], through: &HashSet<u32>, p: Vec2, a: u32, b: u32) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    let (Some(sa), Some(sb)) = (segs.get(a as usize), segs.get(b as usize)) else {
        return a.cmp(&b);
    };
    let geom = match (through.contains(&a), through.contains(&b)) {
        (true, true) => slope_order(sa, sb),
        (true, false) => point_vs_seg(sb, p),
        (false, true) => point_vs_seg(sa, p).reverse(),
        (false, false) => point_vs_seg(sa, sb.left).then(slope_order(sa, sb)),
    };
    geom.then(a.cmp(&b))
}

fn record_pair(raw: &[(Vec2, Vec2)], splits: &mut [Vec<f64>], i: usize, j: usize) {
    let (Some(si), Some(sj)) = (raw.get(i), raw.get(j)) else { return };
    let li = Line::new(si.0, si.1);
    let lj = Line::new(sj.0, sj.1);
    let Some(x) = line_line(&li, &lj) else { return };
    if let Some(v) = splits.get_mut(i) {
        v.push(li.param_of(x).clamp(0.0, 1.0));
    }
    if let Some(v) = splits.get_mut(j) {
        v.push(lj.param_of(x).clamp(0.0, 1.0));
    }
}

/// A crossing worth a vertex: [`line_line`]'s point, or `None` when the segments miss,
/// are parallel, or only share an endpoint that is already a split at 0 or 1.
fn crossing_point(raw: &[(Vec2, Vec2)], i: u32, j: u32) -> Option<Vec2> {
    let (Some(si), Some(sj)) = (raw.get(i as usize), raw.get(j as usize)) else { return None };
    let li = Line::new(si.0, si.1);
    let lj = Line::new(sj.0, sj.1);
    let p = line_line(&li, &lj)?;
    let interior = |t: f64| t > 1e-12 && t < 1.0 - 1e-12;
    (interior(li.param_of(p)) || interior(lj.param_of(p))).then_some(p)
}

struct Job<'a> {
    raw: &'a [(Vec2, Vec2)],
    segs: Vec<Seg>,
    splits: &'a mut [Vec<f64>],
    queue: BinaryHeap<Reverse<Ev>>,
    /// Pairs already queued or recorded, so a neighbour check cannot enqueue the same crossing twice.
    queued_cross: HashSet<(u32, u32)>,
    status: Treap,
    /// Vertical segment ids keyed by the total-order bits of their x.
    verticals: HashMap<u64, Vec<u32>>,
    crossings: usize,
    capped: bool,
}

impl<'a> Job<'a> {
    fn push_cross(&mut self, p: Vec2, a: u32, b: u32) {
        if self.crossings >= MAX_CROSSINGS {
            self.capped = true;
            return;
        }
        let (lo, hi) = if a < b { (a, b) } else { (b, a) };
        if !self.queued_cross.insert((lo, hi)) {
            return;
        }
        self.queue.push(Reverse(Ev { x: total_key(p.x), y: total_key(p.y), kind: Kind::Cross, a: lo, b: hi }));
        self.crossings += 1;
    }

    /// Schedule the crossing of `s` and `t` when it is strictly to the right of `p`.
    /// A crossing at `p` is recorded now. Collinear overlaps and endpoint-only touches are not.
    fn find_new_event(&mut self, s: u32, t: u32, p: Vec2) {
        if s == t || self.capped {
            return;
        }
        let (lo, hi) = if s < t { (s, t) } else { (t, s) };
        if self.queued_cross.contains(&(lo, hi)) {
            return;
        }
        let Some(pt) = crossing_point(self.raw, s, t) else { return };
        if lex_cmp(pt, p) == Ordering::Greater {
            self.push_cross(pt, s, t);
        } else {
            self.queued_cross.insert((lo, hi));
            record_pair(self.raw, self.splits, s as usize, t as usize);
        }
    }

    /// `outer` meets every segment on the same infinite line as `boundary`.
    ///
    /// Parallel segments share an outgoing angle, so the run is the collinear overlap,
    /// not every segment of that slope.
    fn find_against_run(&mut self, outer: u32, boundary: u32, p: Vec2, downward: bool) {
        let Some(anchor) = self.segs.get(boundary as usize).copied() else { return };
        let mut id = boundary;
        let mut guard = 0;
        while guard < self.segs.len() {
            guard += 1;
            if id == outer {
                break;
            }
            let Some(seg) = self.segs.get(id as usize).copied() else { break };
            if id != boundary && !same_line(&anchor, &seg) {
                break;
            }
            self.find_new_event(outer, id, p);
            let Some(nxt) = self.status.neighbor(id, !downward) else { break };
            id = nxt;
        }
    }

    fn scan_verticals(&mut self, xkey: u64) {
        let Some(vs) = self.verticals.get(&xkey).cloned() else { return };
        for vid in vs {
            let Some(v) = self.segs.get(vid as usize).copied() else { continue };
            let mut cur = self.status.first_not_below(&self.segs, v.left);
            let mut guard = 0;
            while let Some(id) = cur {
                if guard > self.segs.len() {
                    break;
                }
                guard += 1;
                let Some(s) = self.segs.get(id as usize).copied() else { break };
                if point_vs_seg(&s, v.right) == Ordering::Less {
                    break;
                }
                if segment_intersects(v.a, v.b, s.a, s.b) {
                    record_pair(self.raw, self.splits, vid as usize, id as usize);
                    self.crossings += 1;
                    if self.crossings >= MAX_CROSSINGS {
                        self.capped = true;
                        return;
                    }
                }
                cur = self.status.neighbor(id, true);
            }
        }
    }

    /// Segments currently in the status whose closed span contains `p`.
    fn through_status(&self, p: Vec2) -> Vec<u32> {
        let mut out = Vec::new();
        let mut cur = self.status.first_not_below(&self.segs, p);
        let mut guard = 0;
        while let Some(id) = cur {
            if guard > self.segs.len() {
                break;
            }
            guard += 1;
            let Some(s) = self.segs.get(id as usize) else { break };
            if point_vs_seg(s, p) == Ordering::Less {
                break;
            }
            if point_on_segment(p, s.left, s.right) {
                out.push(id);
            }
            cur = self.status.neighbor(id, true);
        }
        out
    }

    fn split_verticals_at(&mut self, p: Vec2, upper: &[u32], lower: &[u32]) {
        let Some(vs) = self.verticals.get(&total_key(p.x)).cloned() else { return };
        for vid in vs {
            let Some(v) = self.segs.get(vid as usize).copied() else { continue };
            if !point_on_segment(p, v.left, v.right) {
                continue;
            }
            if let Some(slot) = self.splits.get_mut(vid as usize) {
                slot.push(Line::new(v.a, v.b).param_of(p).clamp(0.0, 1.0));
            }
            for id in upper.iter().chain(lower.iter()) {
                record_pair(self.raw, self.splits, vid as usize, *id as usize);
            }
        }
    }

    fn handle(&mut self, p: Vec2, batch: &[Ev]) {
        let mut upper: Vec<u32> = Vec::new();
        let mut lower: Vec<u32> = Vec::new();
        let mut crossing: Vec<u32> = Vec::new();
        for e in batch {
            match e.kind {
                Kind::Start => upper.push(e.a),
                Kind::End => lower.push(e.a),
                Kind::Cross => {
                    crossing.push(e.a);
                    crossing.push(e.b);
                    record_pair(self.raw, self.splits, e.a as usize, e.b as usize);
                }
                Kind::Column => {}
            }
        }
        let mut on_p = self.through_status(p);
        // The column scan ran before segments that start or end at this x were edited, so an
        // endpoint that lands in the interior of a vertical is recorded here.
        self.split_verticals_at(p, &upper, &lower);

        let mut remove: HashSet<u32> = HashSet::new();
        for id in lower.iter().chain(crossing.iter()).chain(on_p.iter()) {
            if self.status.contains(*id) {
                remove.insert(*id);
            }
        }
        // A start whose endpoint lies in the interior of a status segment: that segment is
        // already in `on_p` when the point test is exact.
        let mut below_above: Vec<(Option<u32>, Option<u32>)> = Vec::new();
        if !remove.is_empty() {
            let mut lows: Vec<u32> = Vec::new();
            let mut highs: Vec<u32> = Vec::new();
            for &id in &remove {
                if self.status.neighbor(id, false).is_none_or(|q| !remove.contains(&q)) {
                    lows.push(id);
                }
                if self.status.neighbor(id, true).is_none_or(|q| !remove.contains(&q)) {
                    highs.push(id);
                }
            }
            // One geometric run is the Bentley–Ottmann case. Keep every boundary the treap
            // reports so a tie-broken collinear run still reconnects its outside neighbours.
            for low in &lows {
                below_above.push((self.status.neighbor(*low, false), None));
            }
            for (slot, high) in below_above.iter_mut().zip(highs.iter()) {
                slot.1 = self.status.neighbor(*high, true);
            }
            if highs.len() > below_above.len() {
                for high in highs.iter().skip(below_above.len()) {
                    below_above.push((None, self.status.neighbor(*high, true)));
                }
            }
        }
        for id in &remove {
            self.status.remove(*id);
        }

        let mut insert: Vec<u32> = Vec::new();
        let mut through: HashSet<u32> = HashSet::new();
        let mut leaving = upper.clone();
        leaving.extend(crossing.iter().copied());
        leaving.append(&mut on_p);
        for id in leaving {
            let continues = self.segs.get(id as usize).is_some_and(|s| !s.vertical && lex_cmp(s.right, p) == Ordering::Greater);
            if continues {
                insert.push(id);
                through.insert(id);
            }
        }
        insert.sort_unstable();
        insert.dedup();
        for id in &insert {
            let segs = &self.segs;
            let through_ref = &through;
            let mut cmp = |a: u32, b: u32| cmp_ids(segs, through_ref, p, a, b);
            self.status.insert(*id, &mut cmp);
        }

        if insert.is_empty() {
            for (below, above) in below_above {
                if let (Some(b), Some(a)) = (below, above) {
                    self.find_against_run(b, a, p, false);
                    self.find_new_event(b, a, p);
                }
            }
            return;
        }
        let inserted: HashSet<u32> = insert.iter().copied().collect();
        for id in &insert {
            if let Some(b) = self.status.neighbor(*id, false)
                && !inserted.contains(&b)
            {
                self.find_against_run(b, *id, p, true);
            }
            if let Some(a) = self.status.neighbor(*id, true)
                && !inserted.contains(&a)
            {
                self.find_against_run(*id, a, p, false);
            }
        }
    }
}

fn same_line(a: &Seg, b: &Seg) -> bool {
    orient2d(a.left, a.right, b.left) == Orientation::Collinear && orient2d(a.left, a.right, b.right) == Orientation::Collinear
}

fn prepare<'a>(raw: &'a [(Vec2, Vec2)], splits: &'a mut [Vec<f64>]) -> Job<'a> {
    let mut segs = Vec::with_capacity(raw.len());
    let mut queue = BinaryHeap::new();
    let mut verticals: HashMap<u64, Vec<u32>> = HashMap::new();
    for (i, &(a, b)) in raw.iter().enumerate() {
        let id = i as u32;
        let swap = lex_cmp(a, b) == Ordering::Greater;
        let (left, right) = if swap { (b, a) } else { (a, b) };
        let vertical = left.x == right.x;
        segs.push(Seg { a, b, left, right, vertical });
        if vertical {
            verticals.entry(total_key(left.x)).or_default().push(id);
            queue.push(Reverse(Ev { x: total_key(left.x), y: total_key(left.y), kind: Kind::Column, a: id, b: 0 }));
        } else {
            queue.push(Reverse(Ev { x: total_key(left.x), y: total_key(left.y), kind: Kind::Start, a: id, b: 0 }));
            queue.push(Reverse(Ev { x: total_key(right.x), y: total_key(right.y), kind: Kind::End, a: id, b: 0 }));
        }
    }
    let n = raw.len();
    Job { raw, segs, splits, queue, queued_cross: HashSet::new(), status: Treap::new(n), verticals, crossings: 0, capped: false }
}

/// Split parameters on each segment, including the endpoints `0` and `1` already in `splits`.
/// `None` when the crossing cap is hit (hostile input) or the slice lengths differ.
pub(crate) fn fill_splits(raw: &[(Vec2, Vec2)], splits: &mut [Vec<f64>]) -> Option<()> {
    if raw.len() != splits.len() {
        return None;
    }
    let mut job = prepare(raw, splits);
    let limit = raw.len().saturating_mul(2).saturating_add(MAX_CROSSINGS);
    let mut steps = 0usize;
    let mut last_x: Option<u64> = None;
    while let Some(Reverse(first)) = job.queue.pop() {
        steps += 1;
        if steps > limit || job.capped {
            return None;
        }
        if last_x != Some(first.x) {
            last_x = Some(first.x);
            job.scan_verticals(first.x);
            if job.capped {
                return None;
            }
        }
        // Leave later points on the heap. `handle` may enqueue a crossing that sorts
        // before the event that was previously next, and a held-aside event would skip it.
        let mut batch = vec![first];
        while job.queue.peek().is_some_and(|Reverse(e)| e.x == first.x && e.y == first.y) {
            let Some(Reverse(e)) = job.queue.pop() else { break };
            batch.push(e);
            steps += 1;
        }
        // Column markers share the point with real events; a pure column batch still runs so
        // an endpoint that lands on a vertical is recorded.
        job.handle(ev_point(first), &batch);
        if job.capped {
            return None;
        }
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_key_roundtrips_and_matches_total_cmp() {
        let samples = [-0.0, 0.0, -1.0, 1.0, f64::MIN, f64::MAX, -1e-300, 1e300, 2.0f64.sqrt()];
        for &a in &samples {
            assert_eq!(from_total_key(total_key(a)).to_bits(), a.to_bits());
        }
        for &a in &samples {
            for &b in &samples {
                assert_eq!(total_key(a).cmp(&total_key(b)), a.total_cmp(&b));
            }
        }
    }
}
