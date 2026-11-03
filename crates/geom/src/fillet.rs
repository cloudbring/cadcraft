//! Circles tangent to two curves (FILLET) and the points where two curves cross.
//!
//! Line/line, line/circle and circle/circle use the closed forms in `intersect`. Every other
//! pair is decomposed into Bézier pieces (Boehm decomposition for splines, exact rational
//! quadratics for circles and ellipses) and intersected by Bézier clipping. A positive radius
//! intersects the offset curves: lines and circles exactly, splines and ellipses by Hermite
//! cubics within a bounded error. Each candidate is polished by Newton with the analytic
//! Jacobian of the two offset equations. `r = 0` is the crossings of the curves themselves.
//! Degenerate input yields no solutions, never a panic or NaN.

use crate::bezier_clip::{self, Bezier, ContactKind};
use crate::{Bounds2, Circle, Ellipse, Line, Spline, TAU, Vec2, circle_circle, line_circle, line_line_infinite};

/// A curve as a tangency target. Lines are infinite, circles and ellipses whole (parameter =
/// angle, eccentric angle for ellipses), splines limited to their domain.
#[derive(Clone, Debug, PartialEq)]
pub enum TanCurve {
    Line(Line),
    Circle(Circle),
    Ellipse(Ellipse),
    Spline(Spline),
}

/// One solution: curve parameters and points where the circle touches each curve, and its
/// centre (equal to the touch points when the radius is zero).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CornerFit {
    pub t1: f64,
    pub t2: f64,
    pub p1: Vec2,
    pub p2: Vec2,
    pub center: Vec2,
}

/// Most solutions reported.
const MAX_SOLUTIONS: usize = 64;

/// Parameter range of a curve; `bounded` ranges clamp (splines end).
#[derive(Clone, Copy, Debug)]
struct Range {
    lo: f64,
    hi: f64,
}

impl TanCurve {
    /// Point at parameter `t`.
    pub fn point(&self, t: f64) -> Vec2 {
        match self {
            TanCurve::Line(l) => l.a + (l.b - l.a) * t,
            TanCurve::Circle(c) => c.center + Vec2::from_angle(t) * c.radius,
            TanCurve::Ellipse(e) => e.at_param(t),
            TanCurve::Spline(s) => s.eval(t),
        }
    }

    /// First derivative at parameter `t`.
    pub fn derivative(&self, t: f64) -> Vec2 {
        match self {
            TanCurve::Line(l) => l.b - l.a,
            TanCurve::Circle(c) => Vec2::from_angle(t).perp() * c.radius,
            TanCurve::Ellipse(e) => e.derivative(t),
            TanCurve::Spline(s) => s.derivative(t),
        }
    }

    /// The point at distance `d` along the left normal at `t` (`None` where the curve stops).
    fn offset(&self, t: f64, d: f64) -> Option<Vec2> {
        let p = self.point(t);
        if d == 0.0 {
            return p.is_finite().then_some(p);
        }
        let n = self.derivative(t).perp().normalized();
        let q = p + n * d;
        (n != Vec2::ZERO && q.is_finite()).then_some(q)
    }

    /// A rough box round a bounded curve.
    fn rough_bounds(&self) -> Option<Bounds2> {
        let b = match self {
            TanCurve::Line(_) => return None,
            TanCurve::Circle(c) => Bounds2::new(c.center - Vec2::new(c.radius, c.radius), c.center + Vec2::new(c.radius, c.radius)),
            TanCurve::Ellipse(e) => Ellipse { start: 0.0, end: TAU, ..*e }.bounds(),
            TanCurve::Spline(s) => Bounds2::from_points(s.control.iter().copied()),
        };
        (!b.is_empty()).then_some(b)
    }

    /// Parameter range to search; infinite lines are cut to `window`.
    fn range(&self, window: &Bounds2) -> Option<Range> {
        match self {
            TanCurve::Line(l) => {
                let d = l.b - l.a;
                let len2 = d.len2();
                if !(len2 > 1e-300 && len2.is_finite()) {
                    return None;
                }
                let ts = window.corners().map(|c| (c - l.a).dot(d) / len2);
                let lo = ts.iter().copied().fold(f64::INFINITY, f64::min);
                let hi = ts.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                (lo.is_finite() && hi.is_finite() && hi > lo).then_some(Range { lo, hi })
            }
            TanCurve::Circle(c) => (c.radius > 1e-300 && c.radius.is_finite()).then_some(Range { lo: 0.0, hi: TAU }),
            TanCurve::Ellipse(e) => (e.major.len() > 1e-300 && e.ratio > 0.0 && e.ratio.is_finite()).then_some(Range { lo: 0.0, hi: TAU }),
            TanCurve::Spline(s) => {
                let (lo, hi) = s.domain();
                (s.is_valid() && lo.is_finite() && hi > lo).then_some(Range { lo, hi })
            }
        }
    }
}

/// Circles of radius `r` tangent to both curves (`r = 0`: the points where they cross). `near`
/// (the pick points) widens the search window for lines. Every distinct solution is returned;
/// callers choose one, usually the nearest the picks.
pub fn tangent_circles(c1: &TanCurve, c2: &TanCurve, r: f64, near: &[Vec2]) -> Vec<CornerFit> {
    if !(r.is_finite() && r >= 0.0) {
        return Vec::new();
    }
    let Some(win) = search_window(c1, c2, near) else { return Vec::new() };
    let size = win.width().max(win.height()).max(r).max(1e-9);
    let win = win.expand(2.0 * r + 0.25 * size);
    let tol = 1e-9 * size.max(1.0);
    if is_line_or_circle(c1) && is_line_or_circle(c2) {
        return closed_forms(c1, c2, r, &win, tol);
    }
    clipped(c1, c2, r, &win, tol)
}

fn is_line_or_circle(c: &TanCurve) -> bool {
    matches!(c, TanCurve::Line(_) | TanCurve::Circle(_))
}

/// The window the tangent search looks in: the pick points and the extents of the curves that
/// are not lines. `None` when it is empty or not finite.
fn search_window(c1: &TanCurve, c2: &TanCurve, near: &[Vec2]) -> Option<Bounds2> {
    let mut win = Bounds2::from_points(near.iter().copied().filter(|p| p.is_finite()));
    for c in [c1, c2] {
        match c {
            TanCurve::Line(l) => {
                win.add(l.a);
                win.add(l.b);
            }
            _ => {
                if let Some(b) = c.rough_bounds() {
                    win = win.union(&b);
                }
            }
        }
    }
    if win.is_empty() || !(win.width().is_finite() && win.height().is_finite()) {
        return None;
    }
    Some(win)
}

fn closed_forms(c1: &TanCurve, c2: &TanCurve, r: f64, win: &Bounds2, tol: f64) -> Vec<CornerFit> {
    match (c1, c2) {
        (TanCurve::Line(a), TanCurve::Line(b)) => line_line_fits(a, b, r, win, tol),
        (TanCurve::Line(l), TanCurve::Circle(c)) => line_circle_fits(l, c, r, win, tol, true),
        (TanCurve::Circle(c), TanCurve::Line(l)) => line_circle_fits(l, c, r, win, tol, false),
        (TanCurve::Circle(a), TanCurve::Circle(b)) => circle_circle_fits(a, b, r, tol),
        _ => Vec::new(),
    }
}

fn line_line_fits(a: &Line, b: &Line, r: f64, win: &Bounds2, tol: f64) -> Vec<CornerFit> {
    let mut out = Vec::new();
    for (s1, s2) in sign_pairs(r) {
        let (o1, o2) = (a.offset(s1 * r), b.offset(s2 * r));
        let Some((center, _, _)) = line_line_infinite(o1.a, o1.b, o2.a, o2.b) else { continue };
        let Some((p1, t1)) = line_touch(a, s1 * r, center) else { continue };
        let Some((p2, t2)) = line_touch(b, s2 * r, center) else { continue };
        if !line_in_window(a, t1, win) || !line_in_window(b, t2, win) {
            continue;
        }
        push_fit(&mut out, CornerFit { t1, t2, p1, p2, center }, tol);
    }
    out
}

fn line_circle_fits(line: &Line, circle: &Circle, r: f64, win: &Bounds2, tol: f64, line_first: bool) -> Vec<CornerFit> {
    let mut out = Vec::new();
    for (s_line, s_circle) in sign_pairs(r) {
        let offset = line.offset(s_line * r);
        let locus = circle.radius - s_circle * r;
        if !locus.is_finite() || locus.abs() <= 1e-14 {
            continue;
        }
        for (center, _) in line_circle(&offset, &Circle::new(circle.center, locus.abs())) {
            let Some((p_line, t_on_line)) = line_touch(line, s_line * r, center) else { continue };
            if !line_in_window(line, t_on_line, win) {
                continue;
            }
            let Some(radial) = circle_radial(circle, s_circle * r, center) else { continue };
            let p_circle = circle.center + radial * circle.radius;
            let t_circle = radial.angle();
            if !p_circle.is_finite() || !center.is_finite() {
                continue;
            }
            let fit = if line_first {
                CornerFit { t1: t_on_line, t2: t_circle, p1: p_line, p2: p_circle, center }
            } else {
                CornerFit { t1: t_circle, t2: t_on_line, p1: p_circle, p2: p_line, center }
            };
            push_fit(&mut out, fit, tol);
        }
    }
    out
}

fn circle_circle_fits(a: &Circle, b: &Circle, r: f64, tol: f64) -> Vec<CornerFit> {
    let mut out = Vec::new();
    for (s1, s2) in sign_pairs(r) {
        let (r1, r2) = (a.radius - s1 * r, b.radius - s2 * r);
        if !r1.is_finite() || !r2.is_finite() || r1.abs() <= 1e-14 || r2.abs() <= 1e-14 {
            continue;
        }
        for center in circle_circle(&Circle::new(a.center, r1.abs()), &Circle::new(b.center, r2.abs())) {
            let (Some(rad1), Some(rad2)) = (circle_radial(a, s1 * r, center), circle_radial(b, s2 * r, center)) else { continue };
            let p1 = a.center + rad1 * a.radius;
            let p2 = b.center + rad2 * b.radius;
            if p1.is_finite() && p2.is_finite() {
                push_fit(&mut out, CornerFit { t1: rad1.angle(), t2: rad2.angle(), p1, p2, center }, tol);
            }
        }
    }
    out
}

fn sign_pairs(r: f64) -> &'static [(f64, f64)] {
    if r > 0.0 { &[(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] } else { &[(0.0, 0.0)] }
}

/// Touch point on `line` for a centre that sits at signed distance `d` along the left normal.
fn line_touch(line: &Line, d: f64, center: Vec2) -> Option<(Vec2, f64)> {
    let n = (line.b - line.a).perp().normalized();
    if n == Vec2::ZERO || !center.is_finite() {
        return None;
    }
    let p = center - n * d;
    p.is_finite().then_some((p, line.param_of(p)))
}

fn line_in_window(line: &Line, t: f64, win: &Bounds2) -> bool {
    let Some(range) = TanCurve::Line(*line).range(win) else { return false };
    t.is_finite() && t >= range.lo - 1e-7 && t <= range.hi + 1e-7
}

/// Unit radial of the touch point. The left normal of a circle points inward, so a positive
/// offset shrinks the radius and a negative locus flips the radial.
fn circle_radial(circle: &Circle, d: f64, center: Vec2) -> Option<Vec2> {
    let v = center - circle.center;
    if v.len2() <= 1e-24 || !v.is_finite() {
        return None;
    }
    let radial = if circle.radius - d < 0.0 { -v } else { v };
    let n = radial.normalized();
    (n != Vec2::ZERO).then_some(n)
}

fn push_fit(out: &mut Vec<CornerFit>, fit: CornerFit, tol: f64) {
    if out.len() >= MAX_SOLUTIONS || !fit.center.is_finite() || !fit.p1.is_finite() || !fit.p2.is_finite() {
        return;
    }
    let gap = tol.max(1e-9) * 1e3;
    if out.iter().any(|f| f.center.near(fit.center, gap) && f.p1.near(fit.p1, gap) && f.p2.near(fit.p2, gap)) {
        return;
    }
    out.push(fit);
}

fn clipped(c1: &TanCurve, c2: &TanCurve, r: f64, win: &Bounds2, tol: f64) -> Vec<CornerFit> {
    let p1 = pieces_of(c1, win);
    let p2 = pieces_of(c2, win);
    if p1.is_empty() || p2.is_empty() {
        return Vec::new();
    }
    let approx = (tol * 40.0).max(1e-6);
    let mut out = Vec::new();
    for (s1, s2) in sign_pairs(r) {
        let (d1, d2) = (s1 * r, s2 * r);
        let o1 = offset_pieces(c1, &p1, d1, approx);
        let o2 = offset_pieces(c2, &p2, d2, approx);
        let accept = if r == 0.0 { tol.clamp(1e-12, 1e-9) } else { (approx * 4.0).max(tol) };
        for a in &o1 {
            for b in &o2 {
                if out.len() >= MAX_SOLUTIONS {
                    return out;
                }
                for hit in bezier_clip::intersect(a, b, accept).contacts {
                    if hit.kind == ContactKind::Overlap {
                        continue;
                    }
                    let hit = HitSeed { at: hit.at, guess: hit.point, radius: r, tol };
                    accept_hit(
                        &OffsetEq { curve: c1, pieces: &p1, distance: d1 },
                        &OffsetEq { curve: c2, pieces: &p2, distance: d2 },
                        &hit,
                        &mut out,
                    );
                }
            }
        }
    }
    out
}

struct HitSeed {
    at: (f64, f64),
    guess: Vec2,
    radius: f64,
    tol: f64,
}

fn accept_hit(a: &OffsetEq, b: &OffsetEq, hit: &HitSeed, out: &mut Vec<CornerFit>) {
    let t_seed = project(a.curve, seed_param(a.curve, a.distance, hit.at.0, hit.guess), hit.guess);
    let u_seed = project(b.curve, seed_param(b.curve, b.distance, hit.at.1, hit.guess), hit.guess);
    let Some((t1, t2)) = newton(a, b, t_seed, u_seed, hit.tol) else { return };
    let (pt1, pt2) = (a.curve.point(t1), b.curve.point(t2));
    let (Some(center), Some(other)) = (a.curve.offset(t1, a.distance), b.curve.offset(t2, b.distance)) else { return };
    let slack = hit.tol.max(1e-8);
    if center.dist(other) > slack || (pt1.dist(center) - hit.radius).abs() > 1e-6 || (pt2.dist(other) - hit.radius).abs() > 1e-6 {
        return;
    }
    push_fit(out, CornerFit { t1, t2, p1: pt1, p2: pt2, center }, hit.tol);
}

fn seed_param(c: &TanCurve, d: f64, hint: f64, center: Vec2) -> f64 {
    match c {
        TanCurve::Line(l) => l.param_of(center - (l.b - l.a).perp().normalized() * d),
        TanCurve::Circle(circle) => circle_radial(circle, d, center).map(|n| n.angle()).unwrap_or(hint),
        TanCurve::Ellipse(_) | TanCurve::Spline(_) => hint,
    }
}

fn project(c: &TanCurve, mut t: f64, point: Vec2) -> f64 {
    if !t.is_finite() {
        t = 0.0;
    }
    for _ in 0..12 {
        let p = c.point(t);
        let v = c.derivative(t);
        let l2 = v.len2();
        if !(l2 > 1e-24 && p.is_finite()) {
            break;
        }
        let step = (point - p).dot(v) / l2;
        if !step.is_finite() || step.abs() < 1e-15 {
            break;
        }
        t = clamp_param(c, t + step.clamp(-1.0, 1.0));
    }
    t
}

fn clamp_param(c: &TanCurve, t: f64) -> f64 {
    match c {
        TanCurve::Spline(s) => {
            let (lo, hi) = s.domain();
            if lo.is_finite() && hi > lo { t.clamp(lo, hi) } else { t }
        }
        _ => t,
    }
}

struct OffsetEq<'a> {
    curve: &'a TanCurve,
    pieces: &'a [Bezier],
    distance: f64,
}

fn newton(a: &OffsetEq, b: &OffsetEq, mut t: f64, mut u: f64, tol: f64) -> Option<(f64, f64)> {
    let mut best: Option<(f64, f64, f64)> = None;
    for _ in 0..16 {
        let (o1, k1) = offset_kin(a, t)?;
        let (o2, k2) = offset_kin(b, u)?;
        let f = o1 - o2;
        let residual = f.len();
        if !residual.is_finite() {
            return None;
        }
        if best.is_none_or(|(_, _, r)| residual < r) {
            best = Some((t, u, residual));
        }
        if residual <= tol.max(1e-12) {
            return Some((t, u));
        }
        let ja = k1;
        let jb = -k2;
        let det = ja.cross(jb);
        if det.abs() <= 1e-14 * ja.len() * jb.len().max(1e-14) {
            break;
        }
        let rhs = -f;
        let (dt, du) = (rhs.cross(jb) / det, ja.cross(rhs) / det);
        if !(dt.is_finite() && du.is_finite()) {
            break;
        }
        t = clamp_param(a.curve, t + dt.clamp(-0.75, 0.75));
        u = clamp_param(b.curve, u + du.clamp(-0.75, 0.75));
    }
    best.filter(|(_, _, r)| *r <= tol.max(1e-8)).map(|(t, u, _)| (t, u))
}

/// Analytic derivative of `point + distance · left_normal`.
fn offset_kin(eq: &OffsetEq, t: f64) -> Option<(Vec2, Vec2)> {
    let (p, v, a) = kinematics(eq.curve, eq.pieces, t)?;
    if eq.distance == 0.0 {
        return (p.is_finite() && v.is_finite()).then_some((p, v));
    }
    let speed = v.len();
    if speed <= 1e-14 {
        return None;
    }
    let n = v.perp() / speed;
    let point = p + n * eq.distance;
    let dn = a.perp() / speed - v.perp() * (v.dot(a) / (speed * speed * speed));
    let deriv = v + dn * eq.distance;
    (point.is_finite() && deriv.is_finite()).then_some((point, deriv))
}

fn kinematics(c: &TanCurve, pieces: &[Bezier], t: f64) -> Option<(Vec2, Vec2, Vec2)> {
    match c {
        TanCurve::Line(l) => {
            let p = l.a + (l.b - l.a) * t;
            p.is_finite().then_some((p, l.b - l.a, Vec2::ZERO))
        }
        TanCurve::Circle(circle) => {
            let radial = Vec2::from_angle(t);
            let p = circle.center + radial * circle.radius;
            let v = radial.perp() * circle.radius;
            (p.is_finite() && v.is_finite()).then_some((p, v, -radial * circle.radius))
        }
        TanCurve::Ellipse(e) => {
            let p = e.at_param(t);
            let v = e.derivative(t);
            // d²/dθ² (center + major cos θ + minor sin θ) = -(point − center).
            (p.is_finite() && v.is_finite()).then_some((p, v, -(p - e.center)))
        }
        TanCurve::Spline(_) => piece_covering(pieces, t)?.eval_parent(t),
    }
}

fn piece_covering(pieces: &[Bezier], t: f64) -> Option<&Bezier> {
    let mut nearest: Option<&Bezier> = None;
    let mut nearest_d = f64::INFINITY;
    for piece in pieces {
        if t >= piece.t0 && t <= piece.t1 {
            return Some(piece);
        }
        let d = (t - piece.t0).abs().min((t - piece.t1).abs());
        if d < nearest_d {
            nearest = Some(piece);
            nearest_d = d;
        }
    }
    nearest.filter(|_| nearest_d <= 1e-5)
}

fn pieces_of(c: &TanCurve, win: &Bounds2) -> Vec<Bezier> {
    match c {
        TanCurve::Line(l) => line_piece(l, win).into_iter().collect(),
        TanCurve::Circle(circle) => {
            conic_pieces(circle.center, Vec2::new(circle.radius, 0.0), Vec2::new(0.0, circle.radius), 0.0, TAU, |p| (p - circle.center).angle())
        }
        TanCurve::Ellipse(e) => {
            let minor = e.minor();
            conic_pieces(e.center, e.major, minor, e.start, e.sweep(), |p| e.param_of(p))
        }
        TanCurve::Spline(s) => spline_pieces(s),
    }
}

fn line_piece(l: &Line, win: &Bounds2) -> Option<Bezier> {
    let range = TanCurve::Line(*l).range(win)?;
    let (a, b) = (l.a + (l.b - l.a) * range.lo, l.a + (l.b - l.a) * range.hi);
    if !a.is_finite() || !b.is_finite() || a.near(b, 1e-15) {
        return None;
    }
    Bezier::polynomial(&[a, b], range.lo, range.hi)
}

/// Rational-quadratic pieces of `center + u cos θ + v sin θ`. Parent parameters are the
/// geometric angle, recovered at the breaks (the Bézier parameter is not linear in θ, and
/// Newton only uses the ends as a bracket).
fn conic_pieces(center: Vec2, u: Vec2, v: Vec2, start: f64, sweep: f64, angle_of: impl Fn(Vec2) -> f64) -> Vec<Bezier> {
    let Some(spline) = Spline::from_conic_arc(center, u, v, start, sweep) else { return Vec::new() };
    let Some((segs, breaks)) = spline.bezier_pieces() else { return Vec::new() };
    let mut out = Vec::new();
    for (seg, span) in segs.iter().zip(breaks.windows(2)) {
        let (Some(&s0), Some(&s1)) = (span.first(), span.get(1)) else { continue };
        let (p0, p1) = (spline.eval(s0), spline.eval(s1));
        if !p0.is_finite() || !p1.is_finite() {
            continue;
        }
        let t0 = angle_of(p0);
        let mut t1 = angle_of(p1);
        if t1 < t0 {
            t1 += TAU;
        }
        if let Some(piece) = Bezier::from_homogeneous(seg, t0, t1) {
            out.push(piece);
        }
    }
    out
}

fn spline_pieces(s: &Spline) -> Vec<Bezier> {
    let Some((segs, breaks)) = s.bezier_pieces() else { return Vec::new() };
    let mut out = Vec::new();
    for (seg, span) in segs.iter().zip(breaks.windows(2)) {
        let (Some(&t0), Some(&t1)) = (span.first(), span.get(1)) else { continue };
        if let Some(piece) = Bezier::from_homogeneous(seg, t0, t1) {
            out.push(piece);
        }
    }
    out
}

fn offset_pieces(c: &TanCurve, pieces: &[Bezier], distance: f64, tol: f64) -> Vec<Bezier> {
    if distance == 0.0 {
        return pieces.to_vec();
    }
    if let TanCurve::Circle(circle) = c {
        return circle_offset_pieces(circle, distance);
    }
    let mut out = Vec::new();
    for piece in pieces {
        out.extend(bezier_clip::offset_curve(piece, distance, tol));
        if out.len() >= 96 {
            break;
        }
    }
    out
}

fn circle_offset_pieces(circle: &Circle, distance: f64) -> Vec<Bezier> {
    let locus = circle.radius - distance;
    if !locus.is_finite() || locus.abs() <= 1e-12 {
        return Vec::new();
    }
    let abs = locus.abs();
    let shift = if locus < 0.0 { crate::PI } else { 0.0 };
    conic_pieces(circle.center, Vec2::new(abs, 0.0), Vec2::new(0.0, abs), shift, TAU, |p| {
        let phi = (p - circle.center).angle();
        if locus < 0.0 { phi - crate::PI } else { phi }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s_curve() -> Spline {
        Spline::from_control(vec![Vec2::new(0.0, 0.0), Vec2::new(3.0, 3.0), Vec2::new(6.0, -3.0), Vec2::new(9.0, 0.0)], 3)
    }

    /// A true fillet: both touch points at distance r from the centre, along each normal.
    fn check(c1: &TanCurve, c2: &TanCurve, f: &CornerFit, r: f64) {
        for (c, t, p) in [(c1, f.t1, f.p1), (c2, f.t2, f.p2)] {
            assert!(c.point(t).near(p, 1e-12));
            assert!((p.dist(f.center) - r).abs() < 1e-7, "{f:?}");
            if r > 0.0 {
                assert!((p - f.center).normalized().dot(c.derivative(t).normalized()).abs() < 1e-6, "{f:?}");
            }
        }
    }

    #[test]
    fn fillet_line_and_ellipse() {
        let e = TanCurve::Ellipse(Ellipse::full(Vec2::ZERO, Vec2::new(4.0, 0.0), 0.5));
        let l = TanCurve::Line(Line::new(Vec2::new(-10.0, 1.0), Vec2::new(10.0, 1.0)));
        let fits = tangent_circles(&e, &l, 0.5, &[Vec2::new(3.0, 1.0)]);
        assert!(fits.len() >= 4, "{fits:?}");
        for f in &fits {
            check(&e, &l, f, 0.5);
        }
        // The crossings: y = 1 meets the ellipse at x = ±4·√(3)/2.
        let x = tangent_circles(&e, &l, 0.0, &[]);
        assert_eq!(x.len(), 2, "{x:?}");
        for f in &x {
            assert!((f.p1.x.abs() - 2.0 * 3f64.sqrt()).abs() < 1e-9 && (f.p1.y - 1.0).abs() < 1e-9);
            assert!(f.p1.near(f.p2, 1e-9));
        }
    }

    #[test]
    fn fillet_spline_with_line_and_circle() {
        let s = TanCurve::Spline(s_curve());
        let l = TanCurve::Line(Line::new(Vec2::new(4.5, -5.0), Vec2::new(4.5, 5.0)));
        let fits = tangent_circles(&s, &l, 0.4, &[]);
        assert!(!fits.is_empty());
        for f in &fits {
            check(&s, &l, f, 0.4);
        }
        let c = TanCurve::Circle(Circle::new(Vec2::new(4.5, 0.0), 1.0));
        let fits = tangent_circles(&s, &c, 0.5, &[]);
        assert!(!fits.is_empty());
        for f in &fits {
            check(&s, &c, f, 0.5);
        }
        // Spline with spline.
        let mirrored = TanCurve::Spline(Spline::from_control(s_curve().control.iter().map(|p| Vec2::new(p.x, 1.0 - p.y)).collect(), 3));
        for f in &tangent_circles(&s, &mirrored, 0.3, &[]) {
            check(&s, &mirrored, f, 0.3);
        }
    }

    #[test]
    fn degenerate_input_gives_nothing() {
        let s = TanCurve::Spline(s_curve());
        let dot = TanCurve::Line(Line::new(Vec2::ZERO, Vec2::ZERO));
        assert!(tangent_circles(&s, &dot, 1.0, &[]).is_empty());
        assert!(tangent_circles(&s, &s, f64::NAN, &[]).is_empty());
        assert!(tangent_circles(&s, &s, -1.0, &[]).is_empty());
        let flat = TanCurve::Ellipse(Ellipse::full(Vec2::ZERO, Vec2::ZERO, 0.5));
        assert!(tangent_circles(&flat, &s, 1.0, &[]).is_empty());
        let huge = TanCurve::Circle(Circle::new(Vec2::new(1e308, 1e308), 1e308));
        let _ = tangent_circles(&huge, &s, 1e308, &[Vec2::new(f64::INFINITY, 0.0)]);
        let bad = TanCurve::Spline(Spline { degree: 3, knots: vec![0.0, 1.0], control: vec![Vec2::ZERO], ..Spline::default() });
        assert!(tangent_circles(&bad, &s, 1.0, &[]).is_empty());
    }

    /// y = (t − 0.37)² as a quadratic Bézier. It touches y = 0 and stays above, so every chord
    /// of a uniform sample stays above the line and the old sampler reported nothing.
    #[test]
    fn tangent_parabola_is_one_contact() {
        let curve = TanCurve::Spline(Spline::from_control(vec![Vec2::new(0.0, 0.1369), Vec2::new(0.5, -0.2331), Vec2::new(1.0, 0.3969)], 2));
        let line = TanCurve::Line(Line::new(Vec2::new(-1.0, 0.0), Vec2::new(2.0, 0.0)));
        let hits = tangent_circles(&curve, &line, 0.0, &[]);
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert!(hits[0].p1.near(Vec2::new(0.37, 0.0), 1e-8), "{hits:?}");
        assert!(hits[0].p1.near(hits[0].p2, 1e-9));
    }

    #[test]
    fn perpendicular_lines_use_the_closed_form() {
        let x = TanCurve::Line(Line::new(Vec2::ZERO, Vec2::X));
        let y = TanCurve::Line(Line::new(Vec2::ZERO, Vec2::Y));
        let fits = tangent_circles(&x, &y, 1.0, &[]);
        assert_eq!(fits.len(), 4, "{fits:?}");
        for f in &fits {
            check(&x, &y, f, 1.0);
            assert!((f.center.x.abs() - 1.0).abs() < 1e-9 && (f.center.y.abs() - 1.0).abs() < 1e-9, "{f:?}");
        }
    }

    #[test]
    fn spline_bezier_derivative_matches_de_boor() {
        let s = s_curve();
        let pieces = spline_pieces(&s);
        assert_eq!(pieces.len(), 1);
        for k in 0..=8 {
            let t = k as f64 / 8.0;
            let (p, v, _) = pieces[0].eval_parent(t).expect("parent");
            assert!(p.near(s.eval(t), 1e-9), "{p:?} vs {:?}", s.eval(t));
            assert!(v.near(s.derivative(t), 1e-8), "{v:?} vs {:?}", s.derivative(t));
        }
    }
}
