//! Certified intersection of rational Bézier curves by Bézier clipping.
//!
//! Sederberg and Nishita, "Curve intersection using Bézier clipping", Computer-Aided
//! Design 22 (1990). Each curve is clipped to the fat line of the other: the signed
//! distances of a curve's control points to the chord of the other (Shewchuk `orient2d`,
//! so the side test is exact) are a Bézier function, and the convex hull of that function
//! bounds every parameter where the curves can still meet. A clip that keeps 80% or more
//! of a parameter interval is not progress — the larger piece is subdivided instead, which
//! separates roots when the fat line is crossed more than once. The search stops at
//! [`MAX_CLIP_NODES`] and [`MAX_CLIP_DEPTH`].
//!
//! A contact that is reached with nearly parallel tangents is one tangency, reported with
//! the parameter bracket still in hand, not two roots. Pieces that coincide are one overlap.
//! Weights must be positive: that is what puts the curve inside the control hull.

use crate::predicates::{Orientation, orient_det, orient2d};
use crate::{Bounds2, Vec2};

/// Most clipping nodes one pair may visit. Past this the search returns what it has.
pub(crate) const MAX_CLIP_NODES: u32 = 8_192;
/// Deepest clip/subdivision chain. A tangency shrinks linearly; this still reaches a
/// geometric tolerance of `1e-12` on a unit-length piece.
pub(crate) const MAX_CLIP_DEPTH: u32 = 160;

/// How two Bézier pieces meet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ContactKind {
    /// The tangents are not parallel. `t` and `u` are the certified bracket.
    Crossing,
    /// A single near-tangent contact. `t` and `u` cover the contact; `at` is one point in it.
    Tangent,
    /// The pieces coincide over `t` and `u`.
    Overlap,
}

/// One intersection of two Bézier curves, in the parent parameters stored on each curve.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Contact {
    /// Bracket on the first curve.
    pub t: (f64, f64),
    /// Bracket on the second curve.
    pub u: (f64, f64),
    /// Parameters of [`point`](Contact::point) inside the brackets.
    pub at: (f64, f64),
    pub point: Vec2,
    pub kind: ContactKind,
}

/// Clipping result. `nodes` counts every region the search entered, and is at most
/// [`MAX_CLIP_NODES`]. `truncated` is set when the node or depth cap stopped the search.
#[derive(Clone, Debug)]
pub(crate) struct ClipReport {
    pub contacts: Vec<Contact>,
    /// Nodes visited. Tests assert this stays at most [`MAX_CLIP_NODES`].
    #[cfg_attr(not(test), allow(dead_code))]
    pub nodes: u32,
    /// Set when the node or depth cap stopped the search before it was finished.
    #[cfg_attr(not(test), allow(dead_code))]
    pub truncated: bool,
}

/// A rational Bézier piece. Homogeneous controls are `(w·P, w)` with `w > 0`.
/// `t0`/`t1` are the parent-curve parameters at the piece's ends (`t1 >= t0`).
#[derive(Clone, Debug)]
pub(crate) struct Bezier {
    h: Vec<(Vec2, f64)>,
    pub t0: f64,
    pub t1: f64,
}

impl Bezier {
    /// Non-rational piece. `None` when a control is not finite or the parameter span is reversed.
    pub(crate) fn polynomial(controls: &[Vec2], t0: f64, t1: f64) -> Option<Self> {
        if controls.len() < 2 || !span_ok(t0, t1) || controls.iter().any(|p| !p.is_finite()) {
            return None;
        }
        Some(Bezier { h: controls.iter().map(|p| (*p, 1.0)).collect(), t0, t1 })
    }

    /// `h` is `(w·P, w)` from [`crate::Spline::bezier_pieces`]. Weights must be positive.
    pub(crate) fn from_homogeneous(h: &[(Vec2, f64)], t0: f64, t1: f64) -> Option<Self> {
        if h.len() < 2 || !span_ok(t0, t1) || h.iter().any(|(p, w)| !(p.is_finite() && w.is_finite() && *w > 0.0)) {
            return None;
        }
        Some(Bezier { h: h.to_vec(), t0, t1 })
    }

    /// Point, first derivative and second derivative in the piece parameter `u` ∈ [0, 1].
    pub(crate) fn eval_deriv2(&self, u: f64) -> Option<(Vec2, Vec2, Vec2)> {
        let u = u.clamp(0.0, 1.0);
        let (xs, ys, ws) = components(&self.h);
        let (x, dx, ddx) = component_derivs(&xs, u)?;
        let (y, dy, ddy) = component_derivs(&ys, u)?;
        let (w, dw, ddw) = component_derivs(&ws, u)?;
        if w.abs() < 1e-300 {
            return None;
        }
        let inv = 1.0 / w;
        let p = Vec2::new(x * inv, y * inv);
        let ax = dx * w - x * dw;
        let ay = dy * w - y * dw;
        let inv2 = inv * inv;
        let dp = Vec2::new(ax * inv2, ay * inv2);
        // A = P′w − P w′, A′ = P″w − P w″, C″ = (A′w − 2 A w′) / w³.
        let apx = ddx * w - x * ddw;
        let apy = ddy * w - y * ddw;
        let inv3 = inv2 * inv;
        let ddp = Vec2::new((apx * w - 2.0 * ax * dw) * inv3, (apy * w - 2.0 * ay * dw) * inv3);
        (p.is_finite() && dp.is_finite() && ddp.is_finite()).then_some((p, dp, ddp))
    }

    /// Value and derivatives in the parent parameter `t`.
    pub(crate) fn eval_parent(&self, t: f64) -> Option<(Vec2, Vec2, Vec2)> {
        let span = self.t1 - self.t0;
        if !(span.is_finite() && span > 1e-300) {
            return None;
        }
        let u = ((t - self.t0) / span).clamp(0.0, 1.0);
        let (p, du, ddu) = self.eval_deriv2(u)?;
        let s = 1.0 / span;
        Some((p, du * s, ddu * s * s))
    }

    fn eval(&self, u: f64) -> Option<Vec2> {
        self.eval_deriv2(u).map(|(p, _, _)| p)
    }

    fn bounds(&self) -> Bounds2 {
        Bounds2::from_points(self.h.iter().filter_map(|h| cartesian(*h)))
    }

    fn diag(&self) -> f64 {
        let b = self.bounds();
        if b.is_empty() { f64::INFINITY } else { b.min.dist(b.max) }
    }

    fn split(&self, u: f64) -> Option<(Bezier, Bezier)> {
        let u = u.clamp(0.0, 1.0);
        let (left, right) = split_h(&self.h, u)?;
        let tm = self.t0 + (self.t1 - self.t0) * u;
        if !(left.len() == self.h.len() && right.len() == self.h.len()) {
            return None;
        }
        Some((Bezier { h: left, t0: self.t0, t1: tm }, Bezier { h: right, t0: tm, t1: self.t1 }))
    }

    fn trim(&self, a: f64, b: f64) -> Option<Bezier> {
        let a = a.clamp(0.0, 1.0);
        let b = b.clamp(a, 1.0);
        if a <= 1e-15 && b >= 1.0 - 1e-15 {
            return Some(self.clone());
        }
        let right = if a > 1e-15 { self.split(a)?.1 } else { self.clone() };
        let local = if (1.0 - a) > 1e-15 { ((b - a) / (1.0 - a)).clamp(0.0, 1.0) } else { 1.0 };
        let piece = if local < 1.0 - 1e-15 { right.split(local)?.0 } else { right };
        Some(piece)
    }

    fn parent_at(&self, u: f64) -> f64 {
        self.t0 + (self.t1 - self.t0) * u.clamp(0.0, 1.0)
    }
}

fn span_ok(t0: f64, t1: f64) -> bool {
    t0.is_finite() && t1.is_finite() && t1 >= t0
}

fn cartesian(h: (Vec2, f64)) -> Option<Vec2> {
    if !(h.0.is_finite() && h.1.is_finite()) || h.1.abs() <= 1e-300 {
        return None;
    }
    let p = h.0 / h.1;
    p.is_finite().then_some(p)
}

fn components(h: &[(Vec2, f64)]) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let xs = h.iter().map(|c| c.0.x).collect();
    let ys = h.iter().map(|c| c.0.y).collect();
    let ws = h.iter().map(|c| c.1).collect();
    (xs, ys, ws)
}

fn lerp_h(a: (Vec2, f64), b: (Vec2, f64), t: f64) -> (Vec2, f64) {
    (a.0 * (1.0 - t) + b.0 * t, a.1 * (1.0 - t) + b.1 * t)
}

/// Left and right homogeneous polygons of a de Casteljau split. The right polygon is
/// stored from the split point toward the end.
fn split_h(h: &[(Vec2, f64)], u: f64) -> Option<(Vec<(Vec2, f64)>, Vec<(Vec2, f64)>)> {
    let mut left = Vec::with_capacity(h.len());
    let mut right = Vec::with_capacity(h.len());
    let mut row = h.to_vec();
    left.push(*row.first()?);
    right.push(*row.last()?);
    while row.len() > 1 {
        let mut next = Vec::with_capacity(row.len() - 1);
        for w in row.windows(2) {
            let (Some(a), Some(b)) = (w.first(), w.get(1)) else { continue };
            next.push(lerp_h(*a, *b, u));
        }
        left.push(*next.first()?);
        right.push(*next.last()?);
        row = next;
    }
    right.reverse();
    Some((left, right))
}

/// Value and the first two derivatives of a polynomial Bézier, parameter `u` ∈ [0, 1].
fn component_derivs(vals: &[f64], u: f64) -> Option<(f64, f64, f64)> {
    if vals.is_empty() || !u.is_finite() {
        return None;
    }
    let n = vals.len() - 1;
    let mut levels: Vec<Vec<f64>> = Vec::with_capacity(n + 1);
    levels.push(vals.to_vec());
    for _ in 0..n {
        let prev = levels.last()?;
        let mut next = Vec::with_capacity(prev.len().saturating_sub(1));
        for w in prev.windows(2) {
            let (Some(a), Some(b)) = (w.first(), w.get(1)) else { continue };
            next.push(a * (1.0 - u) + b * u);
        }
        levels.push(next);
    }
    let v = *levels.last()?.first()?;
    let d1 = if n >= 1 {
        let q = levels.get(n - 1)?;
        n as f64 * (q.get(1).copied()? - q.first().copied()?)
    } else {
        0.0
    };
    let d2 = if n >= 2 {
        let q = levels.get(n - 2)?;
        let (a, b, c) = (q.first().copied()?, q.get(1).copied()?, q.get(2).copied()?);
        (n * (n - 1)) as f64 * (c - 2.0 * b + a)
    } else {
        0.0
    };
    (v.is_finite() && d1.is_finite() && d2.is_finite()).then_some((v, d1, d2))
}

fn convex_hull(pts: &[Vec2]) -> Vec<Vec2> {
    if pts.len() <= 1 {
        return pts.to_vec();
    }
    let mut lower = Vec::new();
    for p in pts {
        while lower.len() >= 2 && !left_turn(&lower, *p) {
            lower.pop();
        }
        lower.push(*p);
    }
    let mut upper = Vec::new();
    for p in pts.iter().rev() {
        while upper.len() >= 2 && !left_turn(&upper, *p) {
            upper.pop();
        }
        upper.push(*p);
    }
    lower.pop();
    upper.pop();
    lower.append(&mut upper);
    if lower.is_empty() { pts.to_vec() } else { lower }
}

fn left_turn(hull: &[Vec2], p: Vec2) -> bool {
    let n = hull.len();
    let (Some(a), Some(b)) = (hull.get(n - 2), hull.get(n - 1)) else { return false };
    orient2d(*a, *b, p) == Orientation::Ccw
}

/// Parameter extent of the convex hull of `(i/n, y_i)` where `y >= 0`.
fn halfplane_t(ys: &[f64]) -> Option<(f64, f64)> {
    if ys.is_empty() || ys.iter().any(|y| !y.is_finite()) {
        return None;
    }
    let n = ys.len() - 1;
    if n == 0 {
        return (ys.first().copied()? >= 0.0).then_some((0.0, 1.0));
    }
    let pts: Vec<Vec2> = ys.iter().enumerate().map(|(i, y)| Vec2::new(i as f64 / n as f64, *y)).collect();
    let hull = convex_hull(&pts);
    if hull.is_empty() {
        return None;
    }
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    let mut touch = |t: f64| {
        if t.is_finite() {
            lo = lo.min(t);
            hi = hi.max(t);
        }
    };
    for i in 0..hull.len() {
        let Some(a) = hull.get(i).copied() else { continue };
        let Some(b) = hull.get((i + 1) % hull.len()).copied() else { continue };
        if a.y >= 0.0 {
            touch(a.x);
        }
        if let Some(t) = cross_zero(a, b) {
            touch(t);
        }
    }
    (lo <= hi).then_some((lo.clamp(0.0, 1.0), hi.clamp(0.0, 1.0)))
}

fn cross_zero(a: Vec2, b: Vec2) -> Option<f64> {
    let dy = b.y - a.y;
    if dy.abs() <= 1e-300 {
        return None;
    }
    let s = (0.0 - a.y) / dy;
    if !(0.0..=1.0).contains(&s) {
        return None;
    }
    let t = a.x + (b.x - a.x) * s;
    t.is_finite().then_some(t)
}

enum Fat {
    Apart,
    /// The chord is too short, or a weight is not positive: subdivide.
    Degenerate,
    Interval(f64, f64),
}

/// Parameter interval of `guest` inside the fat line of `host`.
fn fat_interval(host: &Bezier, guest: &Bezier) -> Fat {
    let Some((a, b)) = ends(host) else { return Fat::Degenerate };
    let chord2 = (b - a).len2();
    if !(chord2.is_finite() && chord2 > 1e-24) {
        return Fat::Degenerate;
    }
    let Some(deltas) = areas(host, a, b) else { return Fat::Degenerate };
    let (Some(dmin), Some(dmax)) = (deltas.iter().copied().reduce(f64::min), deltas.iter().copied().reduce(f64::max)) else {
        return Fat::Degenerate;
    };
    let Some((f, g)) = clearance(guest, a, b, dmin, dmax) else { return Fat::Degenerate };
    if f.iter().chain(g.iter()).any(|y| !y.is_finite()) {
        return Fat::Degenerate;
    }
    let (Some(f_iv), Some(g_iv)) = (halfplane_t(&f), halfplane_t(&g)) else {
        return Fat::Apart;
    };
    let lo = f_iv.0.max(g_iv.0);
    let hi = f_iv.1.min(g_iv.1);
    if hi < lo { Fat::Apart } else { Fat::Interval(lo, hi) }
}

fn ends(curve: &Bezier) -> Option<(Vec2, Vec2)> {
    Some((cartesian(*curve.h.first()?)?, cartesian(*curve.h.last()?)?))
}

fn areas(curve: &Bezier, a: Vec2, b: Vec2) -> Option<Vec<f64>> {
    let mut out = Vec::with_capacity(curve.h.len());
    for h in &curve.h {
        let p = cartesian(*h)?;
        let d = orient_det(a, b, p);
        if !d.is_finite() {
            return None;
        }
        out.push(d);
    }
    Some(out)
}

/// Control values of `W·(δ − δmin)` and `W·(δmax − δ)`. Both are non-negative exactly where
/// the rational curve lies in the fat line, because every weight is positive.
fn clearance(guest: &Bezier, a: Vec2, b: Vec2, dmin: f64, dmax: f64) -> Option<(Vec<f64>, Vec<f64>)> {
    let mut f = Vec::with_capacity(guest.h.len());
    let mut g = Vec::with_capacity(guest.h.len());
    for h in &guest.h {
        if h.1 <= 0.0 {
            return None;
        }
        let p = cartesian(*h)?;
        let delta = orient_det(a, b, p);
        if !delta.is_finite() {
            return None;
        }
        f.push(h.1 * (delta - dmin));
        g.push(h.1 * (dmax - delta));
    }
    Some((f, g))
}

enum ClipTry {
    Apart,
    Stuck,
    Shrunk(Bezier, Bezier),
}

/// Clip each curve to the other's fat line. `Shrunk` only when the survivor is under 80%
/// of some interval — otherwise the caller subdivides (the paper's stall rule).
fn try_clip(a: &Bezier, b: &Bezier) -> ClipTry {
    let b_iv = match fat_interval(a, b) {
        Fat::Apart => return ClipTry::Apart,
        Fat::Degenerate => return ClipTry::Stuck,
        Fat::Interval(lo, hi) => (lo, hi),
    };
    let Some(b2) = b.trim(b_iv.0, b_iv.1) else { return ClipTry::Stuck };
    let a_iv = match fat_interval(&b2, a) {
        Fat::Apart => return ClipTry::Apart,
        Fat::Degenerate => return ClipTry::Stuck,
        Fat::Interval(lo, hi) => (lo, hi),
    };
    let Some(a2) = a.trim(a_iv.0, a_iv.1) else { return ClipTry::Stuck };
    let progress = (b_iv.1 - b_iv.0) < 0.8 || (a_iv.1 - a_iv.0) < 0.8;
    if progress { ClipTry::Shrunk(a2, b2) } else { ClipTry::Stuck }
}

struct Search {
    accept: f64,
    overlap_tol: f64,
    merge_dist: f64,
    nodes: u32,
    truncated: bool,
    out: Vec<Contact>,
}

impl Search {
    fn rec(&mut self, a: &Bezier, b: &Bezier, depth: u32) {
        if self.nodes >= MAX_CLIP_NODES || depth > MAX_CLIP_DEPTH {
            self.truncated = true;
            return;
        }
        self.nodes = self.nodes.saturating_add(1);
        if self.out.len() >= 32 {
            return;
        }
        let (ba, bb) = (a.bounds(), b.bounds());
        if ba.is_empty() || bb.is_empty() || !ba.expand(self.accept).intersects(&bb) {
            return;
        }
        // Once both boxes are this small, Newton (or an endpoint) already certifies a hit.
        // Pushing the fat-line test any further flips the sign of a 1e-18 area and drops
        // the root. A near-miss stays above `accept`, so it is not emitted.
        if a.diag() <= 1e-4 && b.diag() <= 1e-4 {
            let before = self.out.len();
            self.emit_point(a, b);
            if self.out.len() > before {
                return;
            }
        }
        if a.diag() <= self.accept && b.diag() <= self.accept {
            self.emit_point(a, b);
            return;
        }
        if may_overlap(a, b, self.overlap_tol) {
            self.out.push(overlap_contact(a, b));
            return;
        }
        match try_clip(a, b) {
            // Below this size the fat-line area is ~1e-18 and a real crossing can
            // look separated. Subdivide instead; Newton above accepts the root.
            ClipTry::Apart if a.diag() <= 1e-3 && b.diag() <= 1e-3 => self.subdivide(a, b, depth),
            ClipTry::Apart => {}
            ClipTry::Shrunk(a2, b2) => self.rec(&a2, &b2, depth + 1),
            ClipTry::Stuck => self.subdivide(a, b, depth),
        }
    }

    fn subdivide(&mut self, a: &Bezier, b: &Bezier, depth: u32) {
        let split_a = a.diag() >= b.diag();
        let (host, other, host_first) = if split_a { (a, b, true) } else { (b, a, false) };
        let Some((left, right)) = host.split(0.5) else {
            self.truncated = true;
            return;
        };
        if host_first {
            self.rec(&left, other, depth + 1);
            self.rec(&right, other, depth + 1);
        } else {
            self.rec(other, &left, depth + 1);
            self.rec(other, &right, depth + 1);
        }
    }

    fn emit_point(&mut self, a: &Bezier, b: &Bezier) {
        let mut u = 0.5;
        let mut v = 0.5;
        polish(a, b, &mut u, &mut v);
        if tangents_parallel(a, b, u, v) {
            (u, v) = minimize_parallel(a, b);
        }
        // Distances inside a tangency zone are flat down to rounding error, so a
        // point a few ulps off can look closer than the shared endpoint. Ties go
        // to the endpoint, which is the exact control point on a seam.
        let mut best = (u, v, f64::INFINITY, false);
        for (cu, cv, end) in [(u, v, false), (0.0, 0.0, true), (0.0, 1.0, true), (1.0, 0.0, true), (1.0, 1.0, true)] {
            let (Some(pa), Some(pb)) = (a.eval(cu), b.eval(cv)) else { continue };
            let d = pa.dist(pb);
            let closer = d < best.2 - 1e-12;
            let tie = (d - best.2).abs() <= 1e-12 && end && !best.3;
            if closer || tie {
                best = (cu, cv, d, end);
            }
        }
        (u, v) = (best.0, best.1);
        let (Some(pa), Some(pb)) = (a.eval(u), b.eval(v)) else { return };
        if pa.dist(pb) > self.accept {
            return;
        }
        let parallel = tangents_parallel(a, b, u, v);
        let kind = if parallel { ContactKind::Tangent } else { ContactKind::Crossing };
        self.out.push(Contact { t: (a.t0, a.t1), u: (b.t0, b.t1), at: (a.parent_at(u), b.parent_at(v)), point: pa.mid(pb), kind });
    }
}

fn tangents_parallel(a: &Bezier, b: &Bezier, u: f64, v: f64) -> bool {
    let Some((_, da, _)) = a.eval_deriv2(u) else { return true };
    let Some((_, db, _)) = b.eval_deriv2(v) else { return true };
    // `Vec2::normalized` treats anything shorter than `EPS` as zero. A certified
    // bracket is often that short, so compare the raw derivatives.
    let scale = da.len() * db.len();
    if !(scale.is_finite() && scale > 0.0) {
        return true;
    }
    da.dot(db).abs() >= (1.0 - 1e-5) * scale
}

/// A tangency of modest curvature sits inside `tol` over a length of about `sqrt(tol)`.
/// Only a longer coincidence is an overlap; a short one is left for the point test.
fn may_overlap(a: &Bezier, b: &Bezier, tol: f64) -> bool {
    let min_span = (2.0 * tol).sqrt() * 64.0;
    a.diag() > tol * 32.0 && b.diag() > tol * 32.0 && chord_len(a) > min_span && chord_len(b) > min_span && coincident(a, b, tol)
}

fn chord_len(curve: &Bezier) -> f64 {
    match ends(curve) {
        Some((a, b)) => a.dist(b),
        None => 0.0,
    }
}

/// Minimum gap on a tangency. Newton stops where the tangents are parallel, which is
/// the whole contact zone, so the parameter is narrowed by shrinking the bracket.
fn minimize_parallel(a: &Bezier, b: &Bezier) -> (f64, f64) {
    let mut best_u = 0.5;
    let mut best_v = 0.5;
    let mut best_d = f64::INFINITY;
    let mut consider = |u: f64| {
        let Some((d, u2, v2)) = gap_at(a, b, u) else { return };
        if d < best_d {
            best_d = d;
            best_u = u2;
            best_v = v2;
        }
    };
    for k in 0..=8 {
        consider(k as f64 / 8.0);
    }
    let mut lo = 0.0;
    let mut hi = 1.0;
    for _ in 0..40 {
        let m1 = lo + (hi - lo) / 3.0;
        let m2 = hi - (hi - lo) / 3.0;
        let d1 = gap_at(a, b, m1).map(|g| g.0).unwrap_or(f64::INFINITY);
        let d2 = gap_at(a, b, m2).map(|g| g.0).unwrap_or(f64::INFINITY);
        if d1 < d2 { hi = m2 } else { lo = m1 }
    }
    consider(0.5 * (lo + hi));
    (best_u, best_v)
}

fn gap_at(a: &Bezier, b: &Bezier, u: f64) -> Option<(f64, f64, f64)> {
    let p = a.eval(u)?;
    let v = project(b, 0.5, p);
    let q = b.eval(v)?;
    let u2 = project(a, u, q);
    let p2 = a.eval(u2)?;
    let v2 = project(b, v, p2);
    let q2 = b.eval(v2)?;
    Some((p2.dist(q2), u2, v2))
}

/// Newton on `A(u) − B(v) = 0`, clamped to the piece. A singular step (parallel tangents)
/// leaves the closest sample found so far.
fn polish(a: &Bezier, b: &Bezier, u: &mut f64, v: &mut f64) {
    let mut best = (*u, *v, f64::INFINITY);
    for _ in 0..10 {
        let (Some((pa, da, _)), Some((pb, db, _))) = (a.eval_deriv2(*u), b.eval_deriv2(*v)) else { return };
        let f = pa - pb;
        let residual = f.len();
        if residual < best.2 {
            best = (*u, *v, residual);
        }
        if residual <= 1e-15 {
            break;
        }
        let ja = da;
        let jb = -db;
        let det = ja.cross(jb);
        let scale = ja.len() * jb.len();
        if det.abs() <= 1e-14 * scale.max(1.0) {
            break;
        }
        let rhs = -f;
        let (du, dv) = (rhs.cross(jb) / det, ja.cross(rhs) / det);
        if !(du.is_finite() && dv.is_finite()) {
            break;
        }
        *u = (*u + du.clamp(-0.5, 0.5)).clamp(0.0, 1.0);
        *v = (*v + dv.clamp(-0.5, 0.5)).clamp(0.0, 1.0);
    }
    *u = best.0;
    *v = best.1;
}

fn overlap_contact(a: &Bezier, b: &Bezier) -> Contact {
    let point = a.eval(0.5).unwrap_or(Vec2::ZERO).mid(b.eval(0.5).unwrap_or(Vec2::ZERO));
    Contact { t: (a.t0, a.t1), u: (b.t0, b.t1), at: (a.parent_at(0.5), b.parent_at(0.5)), point, kind: ContactKind::Overlap }
}

fn coincident(a: &Bezier, b: &Bezier, tol: f64) -> bool {
    samples_on(a, b, tol) && samples_on(b, a, tol)
}

fn samples_on(src: &Bezier, dst: &Bezier, tol: f64) -> bool {
    for k in 0..=4 {
        let Some(p) = src.eval(k as f64 / 4.0) else { return false };
        if !near_curve(dst, p, tol) {
            return false;
        }
    }
    true
}

fn near_curve(dst: &Bezier, p: Vec2, tol: f64) -> bool {
    if box_gap(&dst.bounds(), p) > tol {
        return false;
    }
    let mut best = f64::INFINITY;
    for k in 0..=8 {
        let u = project(dst, k as f64 / 8.0, p);
        if let Some(q) = dst.eval(u) {
            best = best.min(q.dist(p));
        }
    }
    best <= tol
}

fn box_gap(b: &Bounds2, p: Vec2) -> f64 {
    if b.is_empty() || !p.is_finite() {
        return f64::INFINITY;
    }
    let dx = if p.x < b.min.x {
        b.min.x - p.x
    } else if p.x > b.max.x {
        p.x - b.max.x
    } else {
        0.0
    };
    let dy = if p.y < b.min.y {
        b.min.y - p.y
    } else if p.y > b.max.y {
        p.y - b.max.y
    } else {
        0.0
    };
    dx.hypot(dy)
}

fn project(curve: &Bezier, mut u: f64, point: Vec2) -> f64 {
    for _ in 0..6 {
        let Some((p, d, _)) = curve.eval_deriv2(u) else { break };
        let l2 = d.len2();
        if l2 <= 1e-24 {
            break;
        }
        let step = (point - p).dot(d) / l2;
        if !step.is_finite() || step.abs() < 1e-15 {
            break;
        }
        u = (u + step.clamp(-0.5, 0.5)).clamp(0.0, 1.0);
    }
    u
}

fn merge_contacts(xs: Vec<Contact>, dist: f64) -> Vec<Contact> {
    let mut out: Vec<Contact> = Vec::new();
    for c in xs {
        if let Some(prev) = out.iter_mut().find(|p| same_contact(p, &c, dist)) {
            *prev = union_contact(prev, &c);
        } else {
            out.push(c);
        }
    }
    out
}

fn same_contact(a: &Contact, b: &Contact, dist: f64) -> bool {
    if a.kind == ContactKind::Overlap || b.kind == ContactKind::Overlap {
        return a.kind == ContactKind::Overlap && b.kind == ContactKind::Overlap && intervals_touch(a.t, b.t) && intervals_touch(a.u, b.u);
    }
    if a.point.near(b.point, dist) {
        return true;
    }
    // Subdivision drops a tangency onto both sides of the cut. The polished points
    // land a few ulps apart, farther than `accept` but still one contact.
    a.kind == ContactKind::Tangent && b.kind == ContactKind::Tangent && a.point.near(b.point, dist.max(1e-8))
}

fn intervals_touch(a: (f64, f64), b: (f64, f64)) -> bool {
    let (a0, a1) = (a.0.min(a.1), a.0.max(a.1));
    let (b0, b1) = (b.0.min(b.1), b.0.max(b.1));
    a0 <= b1 + 1e-9 && b0 <= a1 + 1e-9
}

fn union_contact(a: &Contact, b: &Contact) -> Contact {
    let kind = match (a.kind, b.kind) {
        (ContactKind::Overlap, ContactKind::Overlap) => ContactKind::Overlap,
        (ContactKind::Crossing, ContactKind::Crossing) => ContactKind::Crossing,
        _ => ContactKind::Tangent,
    };
    Contact {
        t: union_span(a.t, b.t),
        u: union_span(a.u, b.u),
        at: a.at,
        // A seam reports the same point at both ends of a closed curve. Averaging those
        // samples pulls the point off the exact control point, so keep the first one.
        point: if intervals_touch(a.t, b.t) && intervals_touch(a.u, b.u) { a.point.mid(b.point) } else { a.point },
        kind,
    }
}

/// Join two brackets when they meet. A periodic seam has the same point at both ends of the
/// domain; spanning that gap would report the whole curve as the contact.
fn union_span(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    if intervals_touch(a, b) {
        (a.0.min(b.0), a.1.max(b.1))
    } else if (b.1 - b.0).abs() < (a.1 - a.0).abs() {
        b
    } else {
        a
    }
}

/// Intersections of two rational Bézier curves. `accept` is the largest gap still reported
/// (a gap of `1e-9` with `accept = 1e-12` is a miss). The parameter brackets of a tangency
/// are the pieces the search stopped on.
pub(crate) fn intersect(a: &Bezier, b: &Bezier, accept: f64) -> ClipReport {
    if !(accept.is_finite() && accept > 0.0) {
        return ClipReport { contacts: Vec::new(), nodes: 0, truncated: false };
    }
    let mut search =
        Search { accept, overlap_tol: accept.min(1e-9), merge_dist: 1e-9_f64.min(accept.max(1e-12)), nodes: 0, truncated: false, out: Vec::new() };
    search.rec(a, b, 0);
    let nodes = search.nodes.min(MAX_CLIP_NODES.saturating_add(1));
    ClipReport { contacts: merge_contacts(search.out, search.merge_dist), nodes, truncated: search.truncated }
}

const MAX_OFFSET_DEPTH: u32 = 12;
const MAX_OFFSET_PIECES: usize = 32;

/// Offset of `src` by `distance` along its left normal, as Bézier pieces whose parent
/// parameters match `src`. A line is exact. Anything else is a Hermite cubic, subdivided
/// until it lies within `tol` of the true offset (or until [`MAX_OFFSET_PIECES`]).
pub(crate) fn offset_curve(src: &Bezier, distance: f64, tol: f64) -> Vec<Bezier> {
    let mut out = Vec::new();
    offset_into(src, distance, tol, 0, &mut out);
    out
}

fn offset_into(src: &Bezier, distance: f64, tol: f64, depth: u32, out: &mut Vec<Bezier>) {
    if out.len() >= MAX_OFFSET_PIECES || !tol.is_finite() {
        return;
    }
    if distance == 0.0 {
        out.push(src.clone());
        return;
    }
    if src.h.len() == 2
        && let Some(line) = offset_line(src, distance)
    {
        out.push(line);
        return;
    }
    if depth >= MAX_OFFSET_DEPTH {
        if let Some(piece) = hermite_offset(src, distance).or_else(|| offset_line(src, distance)) {
            out.push(piece);
        }
        return;
    }
    match hermite_offset(src, distance) {
        Some(piece) if offset_error(src, &piece, distance) <= tol => out.push(piece),
        _ => {
            if let Some((left, right)) = src.split(0.5) {
                offset_into(&left, distance, tol, depth + 1, out);
                offset_into(&right, distance, tol, depth + 1, out);
            }
        }
    }
}

fn offset_line(src: &Bezier, distance: f64) -> Option<Bezier> {
    let (p0, _, _) = src.eval_deriv2(0.0)?;
    let (p1, _, _) = src.eval_deriv2(1.0)?;
    let n = (p1 - p0).perp().normalized();
    if n == Vec2::ZERO {
        return None;
    }
    Bezier::polynomial(&[p0 + n * distance, p1 + n * distance], src.t0, src.t1)
}

fn hermite_offset(src: &Bezier, distance: f64) -> Option<Bezier> {
    let (p0, d0) = offset_sample(src, 0.0, distance)?;
    let (p1, d1) = offset_sample(src, 1.0, distance)?;
    let chord = p0.dist(p1).max(1e-12);
    if d0.len() > chord * 12.0 || d1.len() > chord * 12.0 {
        return None;
    }
    Bezier::polynomial(&[p0, p0 + d0 / 3.0, p1 - d1 / 3.0, p1], src.t0, src.t1)
}

fn offset_sample(src: &Bezier, u: f64, distance: f64) -> Option<(Vec2, Vec2)> {
    let (p, v, a) = src.eval_deriv2(u)?;
    let speed = v.len();
    if speed <= 1e-14 {
        return None;
    }
    let n = v.perp() / speed;
    let point = p + n * distance;
    let dn = a.perp() / speed - v.perp() * (v.dot(a) / (speed * speed * speed));
    let deriv = v + dn * distance;
    (point.is_finite() && deriv.is_finite()).then_some((point, deriv))
}

fn offset_error(src: &Bezier, approx: &Bezier, distance: f64) -> f64 {
    let mut worst: f64 = 0.0;
    for k in 1..8 {
        let u = k as f64 / 8.0;
        let Some((p, _)) = offset_sample(src, u, distance) else { return f64::INFINITY };
        let Some((q, _, _)) = approx.eval_deriv2(u) else { return f64::INFINITY };
        let gap = p.dist(q);
        if !gap.is_finite() {
            return f64::INFINITY;
        }
        worst = worst.max(gap);
    }
    worst
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Circle, Spline, TAU, Vec2, circle_circle};
    use proptest::prelude::*;

    fn cubic(c: &[Vec2]) -> Bezier {
        Bezier::polynomial(c, 0.0, 1.0).expect("cubic")
    }

    fn y_only(y: [f64; 4]) -> Bezier {
        cubic(&[Vec2::new(0.0, y[0]), Vec2::new(1.0 / 3.0, y[1]), Vec2::new(2.0 / 3.0, y[2]), Vec2::new(1.0, y[3])])
    }

    fn horizontal(y: f64) -> Bezier {
        Bezier::polynomial(&[Vec2::new(-0.5, y), Vec2::new(1.5, y)], 0.0, 1.0).expect("line")
    }

    /// Roots of a cubic Bernstein function on [0, 1], by sign changes and bisection.
    fn bernstein_roots(y: [f64; 4]) -> Vec<f64> {
        let at = |t: f64| {
            let u = 1.0 - t;
            y[0] * u * u * u + 3.0 * y[1] * u * u * t + 3.0 * y[2] * u * t * t + y[3] * t * t * t
        };
        let mut roots = Vec::new();
        let n = 64;
        for i in 0..n {
            let (t0, t1) = (i as f64 / n as f64, (i + 1) as f64 / n as f64);
            let (f0, f1) = (at(t0), at(t1));
            if f0.abs() <= 1e-15 {
                roots.push(t0);
                continue;
            }
            if f0.signum() == f1.signum() {
                continue;
            }
            let (mut a, mut b) = (t0, t1);
            for _ in 0..60 {
                let m = 0.5 * (a + b);
                if at(a).signum() == at(m).signum() { a = m } else { b = m }
            }
            roots.push(0.5 * (a + b));
        }
        roots.dedup_by(|a, b| (*a - *b).abs() < 1e-10);
        roots
    }

    fn circle_pieces(c: &Circle) -> Vec<Bezier> {
        let spline = Spline::from_conic_arc(c.center, Vec2::new(c.radius, 0.0), Vec2::new(0.0, c.radius), 0.0, TAU).expect("circle spline");
        let (segs, breaks) = spline.bezier_pieces().expect("pieces");
        segs.iter()
            .zip(breaks.windows(2))
            .filter_map(|(seg, w)| {
                let (Some(&t0), Some(&t1)) = (w.first(), w.get(1)) else { return None };
                // Parent parameter is the spline parameter; tests compare points, not angles.
                Bezier::from_homogeneous(seg, t0, t1)
            })
            .collect()
    }

    fn all_hits(a: &[Bezier], b: &[Bezier], tol: f64) -> ClipReport {
        let mut contacts = Vec::new();
        let mut nodes: u32 = 0;
        let mut truncated = false;
        for pa in a {
            for pb in b {
                let r = intersect(pa, pb, tol);
                nodes = nodes.saturating_add(r.nodes);
                truncated |= r.truncated;
                contacts.extend(r.contacts);
            }
        }
        ClipReport { contacts: merge_contacts(contacts, 1e-9), nodes, truncated }
    }

    #[test]
    fn two_circles_match_the_closed_form() {
        let a = Circle::new(Vec2::ZERO, 1.0);
        let b = Circle::new(Vec2::new(1.0, 0.0), 1.0);
        let hits = all_hits(&circle_pieces(&a), &circle_pieces(&b), 1e-12);
        let closed = circle_circle(&a, &b);
        assert_eq!(hits.contacts.len(), closed.len(), "{:?}", hits.contacts.iter().map(|c| c.point).collect::<Vec<_>>());
        assert!(!hits.truncated);
        assert!(hits.nodes <= MAX_CLIP_NODES * 16);
        for h in &hits.contacts {
            assert!(closed.iter().any(|p| p.near(h.point, 1e-9)), "{h:?}");
            assert_eq!(h.kind, ContactKind::Crossing);
        }
    }

    #[test]
    fn external_and_internal_circle_tangencies_are_one_point() {
        let unit = Circle::new(Vec2::ZERO, 1.0);
        let outer = Circle::new(Vec2::new(2.0, 0.0), 1.0);
        let hits = all_hits(&circle_pieces(&unit), &circle_pieces(&outer), 1e-12);
        assert_eq!(hits.contacts.len(), 1, "{:?}", hits.contacts.iter().map(|c| (c.kind, c.point)).collect::<Vec<_>>());
        let h = &hits.contacts[0];
        assert_eq!(h.kind, ContactKind::Tangent);
        assert!(h.point.near(Vec2::new(1.0, 0.0), 1e-9), "{h:?}");
        assert!(h.t.1 >= h.t.0 && h.u.1 >= h.u.0);
        assert!(!hits.truncated);

        let inner = Circle::new(Vec2::new(0.5, 0.0), 0.5);
        let hits = all_hits(&circle_pieces(&unit), &circle_pieces(&inner), 1e-12);
        assert_eq!(hits.contacts.len(), 1, "{:?}", hits.contacts.iter().map(|c| c.point).collect::<Vec<_>>());
        assert_eq!(hits.contacts[0].kind, ContactKind::Tangent);
        assert!(hits.contacts[0].point.near(Vec2::new(1.0, 0.0), 1e-9), "{:?}", hits.contacts[0]);
    }

    #[test]
    fn circle_gap_of_1e_9_is_not_an_intersection() {
        let a = Circle::new(Vec2::ZERO, 1.0);
        let b = Circle::new(Vec2::new(2.0 + 1e-9, 0.0), 1.0);
        let hits = all_hits(&circle_pieces(&a), &circle_pieces(&b), 1e-12);
        assert!(hits.contacts.is_empty(), "{:?}", hits.contacts.iter().map(|c| c.point).collect::<Vec<_>>());
        assert!(!hits.truncated);
    }

    #[test]
    fn cubic_line_roots_match_the_bernstein_polynomial() {
        let y = [-1.0, 2.0, -2.0, 0.5];
        let curve = y_only(y);
        let line = horizontal(0.0);
        let hits = intersect(&curve, &line, 1e-12);
        let roots = bernstein_roots(y);
        assert!(!roots.is_empty());
        assert_eq!(hits.contacts.len(), roots.len(), "clip {hits:?} roots {roots:?}");
        assert!(!hits.truncated);
        assert!(hits.nodes <= MAX_CLIP_NODES);
        for h in &hits.contacts {
            assert!(roots.iter().any(|t| (h.at.0 - t).abs() < 1e-9), "t {} not in {roots:?}", h.at.0);
            assert!(h.point.y.abs() < 1e-9, "{h:?}");
        }
    }

    #[test]
    fn cubic_tangent_to_a_line_is_one_bracket() {
        // y(1/2) = 0 and y′(1/2) = 0, and y stays non-negative: a tangency the chord test misses.
        let y = [1.0, -1.0 / 3.0, -1.0 / 3.0, 1.0];
        let hits = intersect(&y_only(y), &horizontal(0.0), 1e-12);
        assert_eq!(hits.contacts.len(), 1, "{:?}", hits.contacts);
        let h = &hits.contacts[0];
        assert_eq!(h.kind, ContactKind::Tangent);
        assert!((h.at.0 - 0.5).abs() < 1e-8, "{h:?}");
        assert!(h.point.near(Vec2::new(0.5, 0.0), 1e-8), "{h:?}");
        assert!(h.t.1 - h.t.0 >= 0.0);
        assert!(!hits.truncated);
        assert!(hits.nodes <= MAX_CLIP_NODES);
    }

    #[test]
    fn cubic_gap_of_1e_9_above_a_line_is_empty() {
        let y = [1.0 + 1e-9, -1.0 / 3.0 + 1e-9, -1.0 / 3.0 + 1e-9, 1.0 + 1e-9];
        let hits = intersect(&y_only(y), &horizontal(0.0), 1e-12);
        assert!(hits.contacts.is_empty(), "{:?}", hits.contacts);
        assert!(!hits.truncated);
        assert!(hits.nodes <= MAX_CLIP_NODES);
    }

    #[test]
    fn identical_cubics_are_an_overlap_and_stop() {
        let c = cubic(&[Vec2::new(0.0, 0.0), Vec2::new(1.0, 2.0), Vec2::new(2.0, -1.0), Vec2::new(3.0, 0.5)]);
        let hits = intersect(&c, &c, 1e-12);
        assert_eq!(hits.contacts.len(), 1, "{:?}", hits.contacts.iter().map(|c| c.kind).collect::<Vec<_>>());
        assert_eq!(hits.contacts[0].kind, ContactKind::Overlap);
        assert!(!hits.truncated);
        assert!(hits.nodes < 64, "overlap must not subdivide, nodes {}", hits.nodes);
        assert!(hits.nodes <= MAX_CLIP_NODES);
    }

    /// Subdivision used to keep going until the fat-line area underflowed and the
    /// crossing disappeared. This pair is one the dense oracle still sees.
    #[test]
    fn shallow_crossing_survives_subdivision() {
        let a = cubic(&[
            Vec2::new(1.254712280989264, -0.9204781860506189),
            Vec2::new(0.8671477369054695, -1.2629091595573445),
            Vec2::new(0.8129217867954361, -1.3957058634650727),
            Vec2::new(-1.3382343679412563, 1.1577398468709583),
        ]);
        let b = cubic(&[
            Vec2::new(-0.781658604368109, -0.8722078073145747),
            Vec2::new(0.6875697869242021, 0.01913619338306143),
            Vec2::new(0.522579498919805, -0.12727000562719665),
            Vec2::new(-0.6837824216297521, 0.42936959600636604),
        ]);
        let c = cubic(&[
            Vec2::new(1.3086212728388078, 1.456059882715698),
            Vec2::new(0.7999863143399751, -0.28520049118515606),
            Vec2::new(0.36668094260338663, -0.3787558637579785),
            Vec2::new(-1.0982720564309885, 1.0640705986766785),
        ]);
        let d = cubic(&[
            Vec2::new(-1.4876136739173171, 0.3885179860412489),
            Vec2::new(-0.9980987001762988, 1.2341989750283282),
            Vec2::new(1.4911916622392498, -0.9491345754624375),
            Vec2::new(-1.019693912222377, 1.11171023565934),
        ]);
        for (a, b) in [(&a, &b), (&c, &d)] {
            let report = intersect(a, b, 1e-12);
            assert!(!report.truncated, "nodes {}", report.nodes);
            assert!(report.nodes <= MAX_CLIP_NODES);
            let found = oracle(a, b);
            assert!(!found.is_empty(), "{report:?}");
            for p in &found {
                assert!(report.contacts.iter().any(|c| c.point.near(*p, 1e-9)), "{p:?} vs {:?}", report.contacts);
            }
        }
    }

    #[test]
    fn search_never_exceeds_the_node_cap() {
        let a = cubic(&[Vec2::new(0.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(2.0, -1.0), Vec2::new(3.0, 0.2)]);
        let b = cubic(&[Vec2::new(0.0, 0.5), Vec2::new(1.0, -0.5), Vec2::new(2.0, 0.8), Vec2::new(3.0, -0.2)]);
        for pair in [intersect(&a, &b, 1e-12), intersect(&a, &a, 1e-12)] {
            assert!(pair.nodes <= MAX_CLIP_NODES, "{}", pair.nodes);
            assert!(!pair.truncated);
        }
    }

    fn oracle(a: &Bezier, b: &Bezier) -> Vec<Vec2> {
        let n = 240;
        let sa: Vec<Vec2> = (0..=n).map(|i| a.eval(i as f64 / n as f64).unwrap_or(Vec2::ZERO)).collect();
        let sb: Vec<Vec2> = (0..=n).map(|i| b.eval(i as f64 / n as f64).unwrap_or(Vec2::ZERO)).collect();
        let mut hits = Vec::new();
        for i in 0..n {
            for j in 0..n {
                let (Some(&a0), Some(&a1)) = (sa.get(i), sa.get(i + 1)) else { continue };
                let (Some(&b0), Some(&b1)) = (sb.get(j), sb.get(j + 1)) else { continue };
                let Some((fu, fv)) = chord_cross(a0, a1, b0, b1) else { continue };
                let mut u = (i as f64 + fu) / n as f64;
                let mut v = (j as f64 + fv) / n as f64;
                polish(a, b, &mut u, &mut v);
                let (Some(pa), Some(pb)) = (a.eval(u), b.eval(v)) else { continue };
                if pa.dist(pb) <= 1e-12 {
                    let p = pa.mid(pb);
                    if hits.iter().all(|q: &Vec2| q.dist(p) > 1e-8) {
                        hits.push(p);
                    }
                }
            }
        }
        hits
    }

    fn chord_cross(a0: Vec2, a1: Vec2, b0: Vec2, b1: Vec2) -> Option<(f64, f64)> {
        let den = orient_det(Vec2::ZERO, a1 - a0, b1 - b0);
        if den == 0.0 || !den.is_finite() {
            return None;
        }
        let w = b0 - a0;
        let u = orient_det(Vec2::ZERO, w, b1 - b0) / den;
        let v = orient_det(Vec2::ZERO, w, a1 - a0) / den;
        ((-1e-6..=1.0 + 1e-6).contains(&u) && (-1e-6..=1.0 + 1e-6).contains(&v)).then_some((u.clamp(0.0, 1.0), v.clamp(0.0, 1.0)))
    }

    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(48))]
        #[test]
        fn random_cubics_match_a_dense_oracle(coords in proptest::collection::vec(-1.5f64..1.5, 16)) {
            let at = |i: usize| Vec2::new(coords[i * 2], coords[i * 2 + 1]);
            let a = cubic(&[at(0), at(1), at(2), at(3)]);
            let b = cubic(&[at(4), at(5), at(6), at(7)]);
            let report = intersect(&a, &b, 1e-12);
            prop_assert!(report.nodes <= MAX_CLIP_NODES);
            prop_assert!(!report.truncated);
            let found = oracle(&a, &b);
            for p in &found {
                prop_assert!(
                    report.contacts.iter().any(|c| c.point.near(*p, 1e-9)),
                    "missed oracle point {p:?}; contacts {:?}",
                    report.contacts.iter().map(|c| c.point).collect::<Vec<_>>()
                );
            }
            for c in &report.contacts {
                if c.kind == ContactKind::Overlap {
                    continue;
                }
                prop_assert!(near_curve(&a, c.point, 1e-8), "contact off the first curve: {c:?}");
                prop_assert!(near_curve(&b, c.point, 1e-8), "contact off the second curve: {c:?}");
                if c.kind == ContactKind::Crossing {
                    prop_assert!(
                        found.iter().any(|p| p.near(c.point, 1e-9)),
                        "extra crossing {c:?}; oracle {found:?}"
                    );
                }
            }
        }
    }
}
