//! Analytic Jacobian of the constraint residuals.
//!
//! Every [`ConstraintKind`] is differentiated in closed form except spline end frames
//! ([`Form::EndOn`], [`Form::Join`]), which stay on the central-difference Jacobian. Those forms
//! rebuild a spline (and, for a fit spline, run a dense interpolation) before the residual exists,
//! so the partials are not written out. The finite-difference step matches [`crate::lm::jacobian`].
//!
//! At a zero distance the analytic gradient uses the subgradient `(1, 0)`. Central differences
//! there are identically zero and under-rank the radial motion of a point sitting on a centre.

use crate::geom_d::{self, Ctx, D, Dv, dir_n, ray, wrap};
use crate::model::Model;
use crate::residual::{Built, Form};

/// Kinds / forms that still use central differences. Reported by the solver docs and tests.
pub(crate) const NUMERIC_FALLBACK: &[&str] =
    &["Tangent, spline end on a curve (Form::EndOn)", "Tangent or Smooth, joined curve ends including G2 (Form::Join)"];

/// Spline end-frame residuals. Everything else has an analytic partial.
pub(crate) fn is_numeric_fallback(b: &Built) -> bool {
    let fallback = matches!(b.form, Form::EndOn { .. } | Form::Join { .. });
    debug_assert!(!fallback || !NUMERIC_FALLBACK.is_empty());
    fallback
}

/// Row-major Jacobian (`rows × free.len()`), analytic where [`is_numeric_fallback`] is false.
pub(crate) fn dense(model: &Model, built: &[&Built], x: &[f64], free: &[usize]) -> Vec<f64> {
    let e = entries(model, built, x, free);
    e.dense()
}

/// Non-zeros of the Jacobian in free-column coordinates.
pub(crate) struct Entries {
    pub rows: usize,
    pub cols: usize,
    trip: Vec<(u32, u32, f64)>,
}

impl Entries {
    pub(crate) fn dense(&self) -> Vec<f64> {
        let mut m = vec![0.0; self.rows.saturating_mul(self.cols)];
        for &(r, c, v) in self.triplets() {
            let i = r as usize * self.cols + c as usize;
            if let Some(slot) = m.get_mut(i) {
                *slot = v;
            }
        }
        m
    }

    pub(crate) fn triplets(&self) -> &[(u32, u32, f64)] {
        &self.trip
    }
}

pub(crate) fn entries(model: &Model, built: &[&Built], x: &[f64], free: &[usize]) -> Entries {
    let cols = free.len();
    let rows: usize = built.iter().map(|b| b.rows).sum();
    let mut col_of = vec![u32::MAX; x.len()];
    for (c, &j) in free.iter().enumerate() {
        if let Some(slot) = col_of.get_mut(j) {
            *slot = u32::try_from(c).unwrap_or(u32::MAX);
        }
    }
    let mut trip = Vec::new();
    let mut row = 0usize;
    for b in built {
        if is_numeric_fallback(b) {
            push_numeric(model, b, x, &col_of, row, &mut trip);
        } else {
            push_analytic(model, b, x, &col_of, row, &mut trip);
        }
        row += b.rows;
    }
    Entries { rows, cols, trip }
}

fn push_analytic(model: &Model, b: &Built, x: &[f64], col_of: &[u32], row: usize, trip: &mut Vec<(u32, u32, f64)>) {
    for seed in touched(model, b) {
        let Some(&col) = col_of.get(seed) else { continue };
        if col == u32::MAX {
            continue;
        }
        for (k, d) in partials(b, model, x, seed).into_iter().enumerate() {
            if d != 0.0 && d.is_finite() {
                let Some(r) = u32::try_from(row + k).ok() else { continue };
                trip.push((r, col, d));
            }
        }
    }
}

fn push_numeric(model: &Model, b: &Built, x: &[f64], col_of: &[u32], row: usize, trip: &mut Vec<(u32, u32, f64)>) {
    for seed in touched(model, b) {
        let Some(&col) = col_of.get(seed) else { continue };
        if col == u32::MAX {
            continue;
        }
        for (k, d) in finite_difference(b, model, x, seed).into_iter().enumerate() {
            if d != 0.0 && d.is_finite() {
                let Some(r) = u32::try_from(row + k).ok() else { continue };
                trip.push((r, col, d));
            }
        }
    }
}

fn touched(model: &Model, b: &Built) -> Vec<usize> {
    let mut v = Vec::new();
    for r in &b.refs {
        if let Some(s) = model.slot(r.handle) {
            v.extend(s.off..s.off + s.len);
        }
    }
    let n_aux = b.aux_init.len();
    v.extend(b.aux..b.aux.saturating_add(n_aux));
    v.sort_unstable();
    v.dedup();
    v
}

/// Central difference of one constraint, same step as [`crate::lm::jacobian`].
fn finite_difference(b: &Built, model: &Model, x: &[f64], seed: usize) -> Vec<f64> {
    let xj = x.get(seed).copied().unwrap_or(0.0);
    let h = 1e-7 * xj.abs().max(1.0);
    let mut xp = x.to_vec();
    let mut eval_at = |shift: f64| {
        if let Some(v) = xp.get_mut(seed) {
            *v = xj + shift;
        }
        let mut out = Vec::new();
        b.eval(model, &xp, &mut out);
        out
    };
    let rp = eval_at(h);
    let rm = eval_at(-h);
    (0..b.rows)
        .map(|i| {
            let d = (rp.get(i).copied().unwrap_or(0.0) - rm.get(i).copied().unwrap_or(0.0)) / (2.0 * h);
            if d.is_finite() { d } else { 0.0 }
        })
        .collect()
}

fn partials(b: &Built, model: &Model, x: &[f64], seed: usize) -> Vec<f64> {
    let ctx = Ctx { model, x, seed };
    let mut out = eval_d(&ctx, b).unwrap_or_default();
    out.resize(b.rows, D::c(0.0));
    out.into_iter().map(|d| if d.d.is_finite() { d.d } else { 0.0 }).collect()
}

fn eval_d(ctx: &Ctx<'_>, b: &Built) -> Option<Vec<D>> {
    use crate::residual::Form as F;
    use cadcraft_doc::ConstraintKind as K;
    let g = |i: usize| b.refs.get(i);
    match (b.kind, b.form) {
        (_, F::Lock) => Some(Vec::new()),
        (_, F::PE { swap }) => point_ellipse(ctx, b, swap),
        (_, F::LE { swap }) => line_ellipse(ctx, b, swap),
        (_, F::Contact) => contact(ctx, b),
        (_, F::Anchor(anchor)) => anchor_d(ctx, g(0)?, anchor),
        (K::Coincident, F::PP) => coincident_points(ctx, b),
        (K::Coincident, F::PL { swap }) => Some(vec![point_line(ctx, b, swap)?]),
        (K::Coincident, F::PC { swap }) => Some(vec![point_circle(ctx, b, swap)?]),
        (K::Horizontal | K::Vertical, form) => horizontal_vertical(ctx, b, form),
        (K::Parallel | K::Smooth, F::LL) => two_lines(ctx, b, LinePair::Cross),
        (K::Perpendicular, _) => two_lines(ctx, b, LinePair::Dot),
        (K::Collinear, _) => collinear(ctx, b),
        (K::Concentric, _) => concentric(ctx, b),
        (K::Equal, F::LL) => equal_lines(ctx, b),
        (K::Equal, _) => equal_circles(ctx, b),
        (K::Tangent | K::Smooth, F::LC { swap }) => Some(vec![line_circle(ctx, b, swap)?]),
        (K::Tangent | K::Smooth, _) => tangent_circles(ctx, b),
        (K::Symmetric, F::PP) => symmetric_points(ctx, b),
        (K::Symmetric, F::SymLines { crossed }) => symmetric_lines(ctx, b, crossed),
        (K::Symmetric, _) => symmetric_circles(ctx, b),
        (K::Distance(axis), form) => distance(ctx, b, axis, form),
        (K::Angular, F::ArcSweep) => angular_arc(ctx, b),
        (K::Angular, F::Rays { rev }) => angular_rays(ctx, b, rev),
        (K::Radius, _) => radius(ctx, b, 1.0),
        (K::Diameter, _) => radius(ctx, b, 2.0),
        _ => Some(Vec::new()),
    }
}

fn point_line(ctx: &Ctx<'_>, b: &Built, swap: bool) -> Option<D> {
    let (point, line) = if swap { (1, 0) } else { (0, 1) };
    let p = ctx.point(b.refs.get(point)?)?;
    let (s, e) = ctx.line(b.refs.get(line)?)?;
    Some(dir_n(s, e).cross(p.sub(s)))
}

fn point_circle(ctx: &Ctx<'_>, b: &Built, swap: bool) -> Option<D> {
    let (point, curve) = if swap { (1, 0) } else { (0, 1) };
    let p = ctx.point(b.refs.get(point)?)?;
    let (c, r) = ctx.circle(b.refs.get(curve)?)?;
    Some(p.dist(c).sub(r))
}

fn point_ellipse(ctx: &Ctx<'_>, b: &Built, swap: bool) -> Option<Vec<D>> {
    let (point, curve) = if swap { (1, 0) } else { (0, 1) };
    let p = ctx.point(b.refs.get(point)?)?;
    let e = ctx.ellipse(b.refs.get(curve)?)?;
    Some(vec![e.on(p)])
}

fn line_ellipse(ctx: &Ctx<'_>, b: &Built, swap: bool) -> Option<Vec<D>> {
    let (line, curve) = if swap { (1, 0) } else { (0, 1) };
    let (s, e) = ctx.line(b.refs.get(line)?)?;
    let ell = ctx.ellipse(b.refs.get(curve)?)?;
    let direction = dir_n(s, e);
    let signed = direction.cross(ell.c.sub(s)).mul(D::c(b.sign));
    Some(vec![signed.sub(ell.support(direction.perp()))])
}

fn contact(ctx: &Ctx<'_>, b: &Built) -> Option<Vec<D>> {
    let (p0, t0) = curve_at(ctx, b, 0)?;
    let (p1, t1) = curve_at(ctx, b, 1)?;
    let n0 = t0.normalized();
    let n1 = t1.normalized();
    Some(vec![p0.x.sub(p1.x), p0.y.sub(p1.y), n0.cross(n1)])
}

fn curve_at(ctx: &Ctx<'_>, b: &Built, i: usize) -> Option<(Dv, Dv)> {
    let r = b.refs.get(i)?;
    let t = ctx.num(b.aux + i)?;
    if let Some((c, rad)) = ctx.circle(r) {
        return Some(geom_d::circle_at(c, rad, t));
    }
    let e = ctx.ellipse(r)?;
    Some((e.at(t), e.d1(t)))
}

fn anchor_d(ctx: &Ctx<'_>, r: &cadcraft_doc::GeomRef, anchor: cadcraft_geom::Vec2) -> Option<Vec<D>> {
    let p = ctx.point(r)?;
    Some(vec![p.x.sub(D::c(anchor.x)), p.y.sub(D::c(anchor.y))])
}

fn coincident_points(ctx: &Ctx<'_>, b: &Built) -> Option<Vec<D>> {
    let (a, c) = (ctx.point(b.refs.first()?)?, ctx.point(b.refs.get(1)?)?);
    Some(vec![a.x.sub(c.x), a.y.sub(c.y)])
}

fn horizontal_vertical(ctx: &Ctx<'_>, b: &Built, form: Form) -> Option<Vec<D>> {
    use cadcraft_doc::ConstraintKind as K;
    let (first, second) = if form == Form::L1 {
        let r = b.refs.first()?;
        ctx.line(r).or_else(|| ctx.ellipse_axis(r))?
    } else {
        (ctx.point(b.refs.first()?)?, ctx.point(b.refs.get(1)?)?)
    };
    let value = if b.kind == K::Horizontal { first.y.sub(second.y) } else { first.x.sub(second.x) };
    Some(vec![value])
}

enum LinePair {
    Cross,
    Dot,
}

fn two_lines(ctx: &Ctx<'_>, b: &Built, pair: LinePair) -> Option<Vec<D>> {
    let (a0, a1) = ctx.line(b.refs.first()?)?;
    let (c0, c1) = ctx.line(b.refs.get(1)?)?;
    let (u, v) = (dir_n(a0, a1), dir_n(c0, c1));
    let s = match pair {
        LinePair::Cross => u.cross(v),
        LinePair::Dot => u.dot(v),
    };
    Some(vec![s])
}

fn collinear(ctx: &Ctx<'_>, b: &Built) -> Option<Vec<D>> {
    let (a0, a1) = ctx.line(b.refs.first()?)?;
    let (c0, c1) = ctx.line(b.refs.get(1)?)?;
    let direction = dir_n(a0, a1);
    Some(vec![direction.cross(c0.sub(a0)), direction.cross(c1.sub(a0))])
}

fn center_of(ctx: &Ctx<'_>, r: &cadcraft_doc::GeomRef) -> Option<Dv> {
    if let Some((c, _)) = ctx.circle(r) {
        return Some(c);
    }
    if let Some(e) = ctx.ellipse(r) {
        return Some(e.c);
    }
    ctx.point(r)
}

fn concentric(ctx: &Ctx<'_>, b: &Built) -> Option<Vec<D>> {
    let (a, c) = (center_of(ctx, b.refs.first()?)?, center_of(ctx, b.refs.get(1)?)?);
    Some(vec![a.x.sub(c.x), a.y.sub(c.y)])
}

fn equal_lines(ctx: &Ctx<'_>, b: &Built) -> Option<Vec<D>> {
    let (a0, a1) = ctx.line(b.refs.first()?)?;
    let (c0, c1) = ctx.line(b.refs.get(1)?)?;
    Some(vec![a0.dist(a1).sub(c0.dist(c1))])
}

fn equal_circles(ctx: &Ctx<'_>, b: &Built) -> Option<Vec<D>> {
    let (_, r0) = ctx.circle(b.refs.first()?)?;
    let (_, r1) = ctx.circle(b.refs.get(1)?)?;
    Some(vec![r0.sub(r1)])
}

fn line_circle(ctx: &Ctx<'_>, b: &Built, swap: bool) -> Option<D> {
    let (line, curve) = if swap { (1, 0) } else { (0, 1) };
    let (s, e) = ctx.line(b.refs.get(line)?)?;
    let (c, r) = ctx.circle(b.refs.get(curve)?)?;
    Some(dir_n(s, e).cross(c.sub(s)).mul(D::c(b.sign)).sub(r))
}

fn tangent_circles(ctx: &Ctx<'_>, b: &Built) -> Option<Vec<D>> {
    let (c0, r0) = ctx.circle(b.refs.first()?)?;
    let (c1, r1) = ctx.circle(b.refs.get(1)?)?;
    let distance = c0.dist(c1);
    let residual = if b.sign > 0.0 { distance.sub(r0.add(r1)) } else { distance.sub(D::c(b.target).mul(r0.sub(r1))) };
    Some(vec![residual])
}

fn symmetric_points(ctx: &Ctx<'_>, b: &Built) -> Option<Vec<D>> {
    let a = ctx.point(b.refs.first()?)?;
    let c = ctx.point(b.refs.get(1)?)?;
    let (s, e) = ctx.line(b.refs.get(2)?)?;
    let direction = dir_n(s, e);
    Some(vec![direction.cross(a.mid(c).sub(s)), direction.dot(c.sub(a))])
}

fn symmetric_lines(ctx: &Ctx<'_>, b: &Built, crossed: bool) -> Option<Vec<D>> {
    let (a0, a1) = ctx.line(b.refs.first()?)?;
    let (mut c0, mut c1) = ctx.line(b.refs.get(1)?)?;
    if crossed {
        std::mem::swap(&mut c0, &mut c1);
    }
    let (s, e) = ctx.line(b.refs.get(2)?)?;
    let direction = dir_n(s, e);
    Some(vec![direction.cross(a0.mid(c0).sub(s)), direction.dot(c0.sub(a0)), direction.cross(a1.mid(c1).sub(s)), direction.dot(c1.sub(a1))])
}

fn symmetric_circles(ctx: &Ctx<'_>, b: &Built) -> Option<Vec<D>> {
    let (c0, r0) = ctx.circle(b.refs.first()?)?;
    let (c1, r1) = ctx.circle(b.refs.get(1)?)?;
    let (s, e) = ctx.line(b.refs.get(2)?)?;
    let direction = dir_n(s, e);
    Some(vec![direction.cross(c0.mid(c1).sub(s)), direction.dot(c1.sub(c0)), r0.sub(r1)])
}

fn distance(ctx: &Ctx<'_>, b: &Built, axis: cadcraft_doc::DistAxis, form: Form) -> Option<Vec<D>> {
    use cadcraft_doc::DistAxis as A;
    let value = match (axis, form) {
        (_, Form::PL { swap }) => point_line(ctx, b, swap)?,
        _ => {
            let (first, second) =
                if form == Form::L1 { ctx.line(b.refs.first()?)? } else { (ctx.point(b.refs.first()?)?, ctx.point(b.refs.get(1)?)?) };
            match axis {
                A::Aligned => first.dist(second),
                A::Horizontal => second.x.sub(first.x),
                A::Vertical => second.y.sub(first.y),
            }
        }
    };
    Some(vec![value.mul(D::c(b.sign)).sub(D::c(b.target))])
}

fn angular_arc(ctx: &Ctx<'_>, b: &Built) -> Option<Vec<D>> {
    Some(vec![ctx.arc_sweep(b.refs.first()?)?.sub(D::c(b.target))])
}

fn angular_rays(ctx: &Ctx<'_>, b: &Built, rev: [bool; 2]) -> Option<Vec<D>> {
    let (a0, a1) = ctx.line(b.refs.first()?)?;
    let (c0, c1) = ctx.line(b.refs.get(1)?)?;
    let (u, v) = (ray(a0, a1, rev[0]), ray(c0, c1, rev[1]));
    let ang = u.cross(v).atan2(u.dot(v));
    Some(vec![wrap(ang.sub(D::c(b.sign * b.target)))])
}

fn radius(ctx: &Ctx<'_>, b: &Built, scale: f64) -> Option<Vec<D>> {
    let (_, r) = ctx.circle(b.refs.first()?)?;
    Some(vec![r.mul(D::c(scale)).sub(D::c(b.target))])
}

#[cfg(test)]
mod match_tests {
    use std::sync::Arc;

    use super::*;
    use crate::lm;
    use crate::{SolveOptions, build};
    use cadcraft_doc::{
        Arc as DArc, Block, Circle as DCircle, Common, Constraint, ConstraintKind as K, DistAxis, Drawing, Ellipse as DEllipse, Entity, EntityKind,
        GeomRef, Handle, Insert, Line as DLine, LwPolyline, Point as DPoint, Space, Sub,
    };
    use cadcraft_geom::{PolyVertex, Spline as GSpline, Vec2, Vec3};

    fn v(x: f64, y: f64) -> Vec3 {
        Vec3::new(x, y, 0.0)
    }
    fn put(d: &mut Drawing, k: EntityKind) -> Handle {
        d.add(&Space::Model, Common::default(), k).unwrap()
    }
    fn line(d: &mut Drawing, a: (f64, f64), b: (f64, f64)) -> Handle {
        put(d, EntityKind::Line(DLine { a: v(a.0, a.1), b: v(b.0, b.1) }))
    }
    fn circle(d: &mut Drawing, c: (f64, f64), r: f64) -> Handle {
        put(d, EntityKind::Circle(DCircle { center: v(c.0, c.1), radius: r }))
    }
    fn arc(d: &mut Drawing, c: (f64, f64), r: f64, s: f64, e: f64) -> Handle {
        put(d, EntityKind::Arc(DArc { center: v(c.0, c.1), radius: r, start: s, end: e }))
    }
    fn point(d: &mut Drawing, p: (f64, f64)) -> Handle {
        put(d, EntityKind::Point(DPoint { p: v(p.0, p.1), angle: 0.0 }))
    }
    fn ellipse(d: &mut Drawing, c: (f64, f64), major: (f64, f64), ratio: f64, start: f64, end: f64) -> Handle {
        put(d, EntityKind::Ellipse(DEllipse { center: v(c.0, c.1), major: v(major.0, major.1), ratio, start, end }))
    }
    fn spline(d: &mut Drawing, pts: &[(f64, f64)]) -> Handle {
        let s = GSpline::from_control(pts.iter().map(|p| Vec2::new(p.0, p.1)).collect(), 3);
        put(d, EntityKind::Spline(s))
    }
    fn poly(d: &mut Drawing) -> Handle {
        put(
            d,
            EntityKind::LwPolyline(LwPolyline {
                vertices: vec![PolyVertex::new(Vec2::new(0.2, 1.3)), PolyVertex::new(Vec2::new(6.4, 2.1)), PolyVertex::new(Vec2::new(8.0, 7.2))],
                closed: false,
                const_width: 0.0,
                elevation: 0.0,
                plinegen: false,
            }),
        )
    }
    fn con(kind: K, refs: &[GeomRef], expr: &str) -> Constraint {
        Constraint { id: 1, kind, refs: refs.to_vec(), name: String::new(), expr: expr.into() }
    }
    fn w(h: Handle) -> GeomRef {
        GeomRef::whole(h)
    }
    fn s(h: Handle, sub: Sub) -> GeomRef {
        GeomRef::new(h, sub)
    }

    fn max_rel(analytic: &[f64], fd: &[f64]) -> f64 {
        analytic.iter().zip(fd).fold(0.0, |m, (&a, &b)| {
            // Relative tolerance 1e-6, with a 1e-8 absolute noise floor near zero.
            let scale = a.abs().max(b.abs()).max(0.01);
            m.max((a - b).abs() / scale)
        })
    }

    fn jitter(x: &[f64], seed: u64) -> Vec<f64> {
        let mut s = seed;
        x.iter()
            .map(|&v| {
                s = s.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                let u = (s >> 33) as f64 / u32::MAX as f64;
                v + (u - 0.5) * 0.3
            })
            .collect()
    }

    /// Analytic partials against central differences at the built parameters and four jitters.
    fn assert_jac(label: &str, d: &Drawing, c: &Constraint) {
        let cons = [c];
        let (comps, skip) = build(d, &cons, &SolveOptions::default()).unwrap_or_else(|e| panic!("{label}: {e}"));
        assert!(skip.is_empty(), "{label}: {skip:?}");
        let comp = &comps[0];
        let built: Vec<&Built> = comp.built.iter().collect();
        assert!(!built.is_empty(), "{label}");
        for (i, x) in std::iter::once(comp.model.x0.clone()).chain((1..5).map(|k| jitter(&comp.model.x0, 0xA11CE + k * 17))).enumerate() {
            let free = comp.free(None);
            let rows = comp.rows(None);
            let e = entries(&comp.model, &built, &x, &free);
            let _ = e.triplets();
            let an = e.dense();
            let f = |xx: &[f64], out: &mut Vec<f64>| comp.eval(xx, out, None);
            let fd = lm::jacobian(&f, &x, &free, rows);
            assert_eq!(an.len(), fd.len(), "{label}");
            let err = max_rel(&an, &fd);
            assert!(err <= 1e-6, "{label} sample {i} rel {err}\nan {an:?}\nfd {fd:?}");
        }
    }

    fn forms_of(d: &Drawing, c: &Constraint) -> Vec<Form> {
        let cons = [c];
        let (comps, skipped) = build(d, &cons, &SolveOptions::default()).unwrap();
        assert!(skipped.is_empty(), "{skipped:?}");
        assert_eq!(comps.len(), 1);
        comps[0].built.iter().map(|b| b.form).collect()
    }

    #[test]
    fn coincident_jacobian_matches_central_difference() {
        let mut d = Drawing::default();
        let a = line(&mut d, (0.2, 1.1), (8.4, 2.3));
        let b = line(&mut d, (1.5, 6.0), (7.2, 0.4));
        let p = point(&mut d, (3.1, 4.4));
        let q = point(&mut d, (9.0, 1.2));
        let c = circle(&mut d, (2.0, 2.5), 3.5);
        let e = ellipse(&mut d, (1.0, -2.0), (4.2, 1.1), 0.45, 0.0, std::f64::consts::TAU);
        let ar = arc(&mut d, (4.0, 5.0), 2.5, 0.4, 2.1);
        let pl = poly(&mut d);
        let pp = con(K::Coincident, &[s(a, Sub::Start), s(b, Sub::End)], "");
        assert_eq!(forms_of(&d, &pp), vec![Form::PP]);
        assert_jac("coincident points", &d, &pp);
        let mid = con(K::Coincident, &[s(a, Sub::Mid), w(p)], "");
        assert_jac("midpoint", &d, &mid);
        let pln = con(K::Coincident, &[w(q), w(a)], "");
        assert!(matches!(forms_of(&d, &pln)[0], Form::PL { .. }));
        assert_jac("point on line", &d, &pln);
        let pc = con(K::Coincident, &[w(p), w(c)], "");
        assert!(matches!(forms_of(&d, &pc)[0], Form::PC { .. }));
        assert_jac("point on circle", &d, &pc);
        let pe = con(K::Coincident, &[w(q), w(e)], "");
        assert!(matches!(forms_of(&d, &pe)[0], Form::PE { .. }));
        assert_jac("point on ellipse", &d, &pe);
        let ap = con(K::Coincident, &[s(ar, Sub::Start), w(p)], "");
        assert_jac("arc start", &d, &ap);
        let am = con(K::Coincident, &[s(ar, Sub::Mid), w(q)], "");
        assert_jac("arc mid", &d, &am);
        let vx = con(K::Coincident, &[s(pl, Sub::Vertex(1)), w(p)], "");
        assert_jac("polyline vertex", &d, &vx);
        let ev = con(K::Coincident, &[s(e, Sub::Vertex(1)), w(p)], "");
        assert_jac("ellipse vertex", &d, &ev);
    }

    #[test]
    fn insert_vertex_jacobian_matches_central_difference() {
        let mut d = Drawing::default();
        let mut b = Block::new("BAR");
        let h = d.new_handle();
        b.entities.push(Entity { handle: h, common: Common::default(), kind: EntityKind::Line(DLine { a: v(0.0, 0.0), b: v(4.0, 1.5) }) });
        d.blocks.insert("BAR".into(), Arc::new(b));
        let ins = put(
            &mut d,
            EntityKind::Insert(Insert {
                block: "BAR".into(),
                insert: v(1.2, 2.4),
                scale: v(1.0, 1.0),
                rotation: 0.4,
                attribs: Vec::new(),
                cols: 1,
                rows: 1,
                col_spacing: 0.0,
                row_spacing: 0.0,
            }),
        );
        let p = point(&mut d, (6.0, 3.0));
        let c = con(K::Coincident, &[s(ins, Sub::Vertex(1)), w(p)], "");
        assert_jac("insert vertex", &d, &c);
    }

    #[test]
    fn horizontal_vertical_jacobian_matches_central_difference() {
        let mut d = Drawing::default();
        let a = line(&mut d, (0.4, 1.2), (7.5, 3.3));
        let p = point(&mut d, (1.0, 4.0));
        let q = point(&mut d, (5.5, 2.2));
        let e = ellipse(&mut d, (2.0, 3.0), (3.5, 1.2), 0.5, 0.0, std::f64::consts::TAU);
        let h = con(K::Horizontal, &[w(a)], "");
        assert_eq!(forms_of(&d, &h), vec![Form::L1]);
        assert_jac("horizontal line", &d, &h);
        let vv = con(K::Vertical, &[w(p), w(q)], "");
        assert_eq!(forms_of(&d, &vv), vec![Form::P2]);
        assert_jac("vertical points", &d, &vv);
        let he = con(K::Horizontal, &[w(e)], "");
        assert_jac("horizontal ellipse axis", &d, &he);
        let pl = poly(&mut d);
        let hs = con(K::Horizontal, &[s(pl, Sub::Segment(0))], "");
        assert_jac("horizontal segment", &d, &hs);
    }

    #[test]
    fn parallel_perpendicular_collinear_jacobian_matches_central_difference() {
        let mut d = Drawing::default();
        let a = line(&mut d, (0.3, 0.6), (8.0, 1.4));
        let b = line(&mut d, (1.0, 4.0), (7.5, 6.2));
        let c = line(&mut d, (2.0, 0.2), (3.1, 7.4));
        let par = con(K::Parallel, &[w(a), w(b)], "");
        assert_jac("parallel", &d, &par);
        let per = con(K::Perpendicular, &[w(a), w(c)], "");
        assert_jac("perpendicular", &d, &per);
        let col = con(K::Collinear, &[w(a), w(b)], "");
        assert_jac("collinear", &d, &col);
    }

    #[test]
    fn concentric_equal_jacobian_matches_central_difference() {
        let mut d = Drawing::default();
        let c1 = circle(&mut d, (1.2, 2.2), 3.0);
        let c2 = circle(&mut d, (2.4, 0.8), 1.7);
        let e = ellipse(&mut d, (0.5, 3.5), (4.0, 0.8), 0.4, 0.0, std::f64::consts::TAU);
        let a = line(&mut d, (0.2, 1.0), (6.5, 2.4));
        let b = line(&mut d, (1.0, 4.0), (8.0, 5.5));
        let cc = con(K::Concentric, &[w(c1), w(e)], "");
        assert_jac("concentric ellipse", &d, &cc);
        let eq = con(K::Equal, &[w(c1), w(c2)], "");
        assert_jac("equal circles", &d, &eq);
        let el = con(K::Equal, &[w(a), w(b)], "");
        assert_eq!(forms_of(&d, &el), vec![Form::LL]);
        assert_jac("equal lines", &d, &el);
    }

    #[test]
    fn tangent_jacobian_matches_central_difference() {
        let mut d = Drawing::default();
        let l = line(&mut d, (0.2, 4.5), (9.0, 5.2));
        let c = circle(&mut d, (3.0, 1.0), 2.2);
        let a = arc(&mut d, (8.0, 2.0), 1.8, 0.3, 2.4);
        let e = ellipse(&mut d, (2.0, 0.5), (3.2, 0.6), 0.45, 0.0, std::f64::consts::TAU);
        let lc = con(K::Tangent, &[w(l), w(c)], "");
        assert!(matches!(forms_of(&d, &lc)[0], Form::LC { .. }));
        assert_jac("line-circle tangent", &d, &lc);
        let le = con(K::Tangent, &[w(l), w(e)], "");
        assert!(matches!(forms_of(&d, &le)[0], Form::LE { .. }));
        assert_jac("line-ellipse tangent", &d, &le);
        let cc = con(K::Tangent, &[w(c), w(a)], "");
        assert_eq!(forms_of(&d, &cc), vec![Form::CC]);
        assert_jac("circle tangent", &d, &cc);
        let e2 = ellipse(&mut d, (7.5, 3.0), (2.4, 1.0), 0.55, 0.2, 2.2);
        let ct = con(K::Tangent, &[w(c), w(e2)], "");
        assert_eq!(forms_of(&d, &ct), vec![Form::Contact]);
        assert_jac("circle-ellipse contact", &d, &ct);
        let l2 = line(&mut d, (0.4, 0.2), (2.0, 6.5));
        let sm = con(K::Smooth, &[w(l), w(l2)], "");
        assert_eq!(forms_of(&d, &sm), vec![Form::LL]);
        assert_jac("smooth lines", &d, &sm);
        let sa = con(K::Smooth, &[w(l2), w(a)], "");
        assert_jac("smooth line-arc", &d, &sa);
    }

    #[test]
    fn symmetric_fix_jacobian_matches_central_difference() {
        let mut d = Drawing::default();
        let axis = line(&mut d, (0.0, -1.0), (0.3, 8.0));
        let p = point(&mut d, (-3.2, 2.0));
        let q = point(&mut d, (3.4, 2.6));
        let a = line(&mut d, (-4.0, 1.0), (-1.5, 4.0));
        let b = line(&mut d, (4.2, 1.3), (1.6, 4.4));
        let c1 = circle(&mut d, (-2.5, 5.0), 1.4);
        let c2 = circle(&mut d, (2.8, 4.6), 1.1);
        let sp = con(K::Symmetric, &[w(p), w(q), w(axis)], "");
        assert_eq!(forms_of(&d, &sp), vec![Form::PP]);
        assert_jac("symmetric points", &d, &sp);
        let sl = con(K::Symmetric, &[w(a), w(b), w(axis)], "");
        assert!(matches!(forms_of(&d, &sl)[0], Form::SymLines { .. }));
        assert_jac("symmetric lines", &d, &sl);
        let sc = con(K::Symmetric, &[w(c1), w(c2), w(axis)], "");
        assert_jac("symmetric circles", &d, &sc);
        let ar = arc(&mut d, (3.0, 3.0), 2.0, 0.5, 2.0);
        let fx = con(K::Fix, &[s(ar, Sub::Start)], "");
        assert!(matches!(forms_of(&d, &fx)[0], Form::Anchor(_)));
        assert_jac("fix anchor", &d, &fx);
        let fl = con(K::Fix, &[w(axis)], "");
        assert_eq!(forms_of(&d, &fl), vec![Form::Lock]);
        assert_jac("fix lock", &d, &fl);
    }

    #[test]
    fn distance_angle_radius_jacobian_matches_central_difference() {
        let mut d = Drawing::default();
        let a = line(&mut d, (0.3, 1.1), (8.2, 2.6));
        let b = line(&mut d, (1.0, 0.4), (2.2, 7.5));
        let p = point(&mut d, (3.0, 5.5));
        let q = point(&mut d, (7.4, 1.8));
        let c = circle(&mut d, (2.0, 2.0), 3.3);
        let ar = arc(&mut d, (1.0, 1.0), 4.0, 0.35, 1.9);
        let da = con(K::Distance(DistAxis::Aligned), &[w(a)], "6");
        assert_jac("distance line", &d, &da);
        let dh = con(K::Distance(DistAxis::Horizontal), &[w(p), w(q)], "4");
        assert_jac("distance horizontal", &d, &dh);
        let dv = con(K::Distance(DistAxis::Vertical), &[w(p), w(q)], "3");
        assert_jac("distance vertical", &d, &dv);
        let dp = con(K::Distance(DistAxis::Aligned), &[w(p), w(a)], "2");
        assert!(matches!(forms_of(&d, &dp)[0], Form::PL { .. }));
        assert_jac("distance point-line", &d, &dp);
        let ang = con(K::Angular, &[w(a), w(b)], "70");
        assert!(matches!(forms_of(&d, &ang)[0], Form::Rays { .. }));
        assert_jac("angular rays", &d, &ang);
        let sw = con(K::Angular, &[w(ar)], "80");
        assert_eq!(forms_of(&d, &sw), vec![Form::ArcSweep]);
        assert_jac("angular arc", &d, &sw);
        let r = con(K::Radius, &[w(c)], "2.5");
        assert_jac("radius", &d, &r);
        let dia = con(K::Diameter, &[w(ar)], "5");
        assert_jac("diameter", &d, &dia);
    }

    #[test]
    fn spline_end_frames_use_numeric_fallback() {
        assert_eq!(NUMERIC_FALLBACK.len(), 2);
        assert!(NUMERIC_FALLBACK.iter().any(|s| s.contains("EndOn")));
        assert!(NUMERIC_FALLBACK.iter().any(|s| s.contains("Join")));
        let mut d = Drawing::default();
        let sp = spline(&mut d, &[(2.4, 0.6), (3.2, 2.5), (6.0, 3.4), (9.0, 2.1)]);
        let c = circle(&mut d, (0.0, 0.0), 2.2);
        let end = con(K::Tangent, &[w(sp), w(c)], "");
        assert!(matches!(forms_of(&d, &end)[0], Form::EndOn { .. }));
        assert_jac("spline end on circle", &d, &end);
        let a = spline(&mut d, &[(0.0, 0.2), (1.5, 2.4), (3.2, 2.0), (5.0, 0.4)]);
        let b = spline(&mut d, &[(5.3, 0.2), (7.0, -1.4), (9.2, 1.1), (11.0, 0.6)]);
        let join = con(K::Smooth, &[w(a), w(b)], "");
        assert!(matches!(forms_of(&d, &join)[0], Form::Join { .. }));
        assert_jac("spline join", &d, &join);
        let tangent_join = con(K::Tangent, &[w(a), w(b)], "");
        assert!(matches!(forms_of(&d, &tangent_join)[0], Form::Join { g2: false, .. }));
        assert_jac("spline G1 join", &d, &tangent_join);
        let joined_line = line(&mut d, (-1.0, 0.4), (2.0, 0.5));
        let smooth_line = con(K::Smooth, &[w(joined_line), w(sp)], "");
        assert!(matches!(forms_of(&d, &smooth_line)[0], Form::Join { g2: true, .. }));
        assert_jac("smooth spline-line join", &d, &smooth_line);
        let vtx = point(&mut d, (3.0, 2.0));
        let on_v = con(K::Coincident, &[s(sp, Sub::Vertex(1)), w(vtx)], "");
        let cons = [&on_v];
        let (comps, _) = build(&d, &cons, &SolveOptions::default()).unwrap();
        assert!(comps[0].built.iter().all(|b| !is_numeric_fallback(b)));
        assert_jac("spline vertex coincident", &d, &on_v);
    }

    #[test]
    fn reversed_and_internal_branches_match_central_difference() {
        let mut drawing = Drawing::default();
        let first = line(&mut drawing, (0.3, 0.7), (6.2, 2.4));
        let second = line(&mut drawing, (1.4, 5.3), (7.3, 6.6));
        let vertex = point(&mut drawing, (4.6, -2.3));
        let outer = circle(&mut drawing, (2.1, 1.3), 4.7);
        let inner = circle(&mut drawing, (3.2, 1.7), 1.4);
        let oval = ellipse(&mut drawing, (8.1, -1.3), (3.2, 1.1), 0.7, 0.3, 2.4);
        for constraint in [
            con(K::Coincident, &[w(first), w(vertex)], ""),
            con(K::Coincident, &[w(outer), w(vertex)], ""),
            con(K::Coincident, &[w(oval), w(vertex)], ""),
            con(K::Tangent, &[w(outer), w(first)], ""),
            con(K::Tangent, &[w(oval), w(first)], ""),
            con(K::Tangent, &[w(outer), w(inner)], ""),
            con(K::Tangent, &[w(inner), w(outer)], ""),
            con(K::Smooth, &[w(outer), w(inner)], ""),
            con(K::Smooth, &[w(oval), w(outer)], ""),
            con(K::Distance(DistAxis::Aligned), &[w(first), w(vertex)], "2"),
            con(K::Distance(DistAxis::Aligned), &[s(first, Sub::End), s(first, Sub::Start)], "2"),
            con(K::Distance(DistAxis::Horizontal), &[w(second)], "2"),
            con(K::Distance(DistAxis::Vertical), &[w(second)], "2"),
            con(K::Vertical, &[w(oval)], ""),
            con(K::Coincident, &[s(oval, Sub::Mid), w(vertex)], ""),
            con(K::Coincident, &[s(oval, Sub::End), w(vertex)], ""),
        ] {
            assert_jac(constraint.kind.name(), &drawing, &constraint);
        }
        let reversed = line(&mut drawing, (-2.7, 8.1), (0.2, 0.8));
        let angular = con(K::Angular, &[w(first), w(reversed)], "70");
        assert!(matches!(forms_of(&drawing, &angular)[0], Form::Rays { rev: [false, true] }));
        assert_jac("reversed angular ray", &drawing, &angular);
    }
}
