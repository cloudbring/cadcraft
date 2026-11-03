//! Forward-mode derivatives of the geometry the residuals read.
//!
//! One parameter is seeded (`d = 1`) per evaluation. Branches follow the value, matching
//! [`crate::model`] and [`crate::curves`], so a partial is the derivative of the same expression
//! the residual uses.

use cadcraft_doc::{GeomRef, Sub};
use cadcraft_geom::{Vec2, ccw_sweep};

use crate::model::{Model, Shape};

const TAU: f64 = std::f64::consts::TAU;
const PI: f64 = std::f64::consts::PI;

/// Value and derivative with respect to the seeded parameter.
#[derive(Clone, Copy, Debug)]
pub(crate) struct D {
    pub v: f64,
    pub d: f64,
}

impl D {
    pub(crate) fn c(v: f64) -> Self {
        D { v, d: 0.0 }
    }
    pub(crate) fn add(self, o: Self) -> Self {
        D { v: self.v + o.v, d: self.d + o.d }
    }
    pub(crate) fn sub(self, o: Self) -> Self {
        D { v: self.v - o.v, d: self.d - o.d }
    }
    pub(crate) fn mul(self, o: Self) -> Self {
        D { v: self.v * o.v, d: self.d * o.v + self.v * o.d }
    }
    pub(crate) fn neg(self) -> Self {
        D { v: -self.v, d: -self.d }
    }
    pub(crate) fn recip(self) -> Self {
        let v = 1.0 / self.v;
        D { v, d: if self.v == 0.0 { 0.0 } else { -self.d / (self.v * self.v) } }
    }
    pub(crate) fn div(self, o: Self) -> Self {
        self.mul(o.recip())
    }
    /// `hypot`, with subgradient `(1, 0)` at the origin (finite differences report 0 there).
    pub(crate) fn hypot(self, o: Self) -> Self {
        let v = self.v.hypot(o.v);
        if v > 1e-15 { D { v, d: (self.v * self.d + o.v * o.d) / v } } else { D { v, d: self.d } }
    }
    pub(crate) fn abs(self) -> Self {
        if self.v > 0.0 {
            self
        } else if self.v < 0.0 {
            self.neg()
        } else {
            D { v: 0.0, d: 0.0 }
        }
    }
    pub(crate) fn sqrt(self) -> Self {
        let v = self.v.sqrt();
        D { v, d: if v > 1e-15 && v.is_finite() { self.d / (2.0 * v) } else { 0.0 } }
    }
    pub(crate) fn sin(self) -> Self {
        D { v: self.v.sin(), d: self.v.cos() * self.d }
    }
    pub(crate) fn cos(self) -> Self {
        D { v: self.v.cos(), d: -self.v.sin() * self.d }
    }
    /// `y.atan2(x)` with `self` as `y`.
    pub(crate) fn atan2(self, x: Self) -> Self {
        let v = self.v.atan2(x.v);
        let n = x.v * x.v + self.v * self.v;
        let d = if n > 1e-30 { (x.v * self.d - self.v * x.d) / n } else { 0.0 };
        D { v, d }
    }
}

/// A point or vector and its derivative.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Dv {
    pub x: D,
    pub y: D,
}

impl Dv {
    fn new(x: D, y: D) -> Self {
        Dv { x, y }
    }
    pub(crate) fn c(p: Vec2) -> Self {
        Dv::new(D::c(p.x), D::c(p.y))
    }
    pub(crate) fn add(self, o: Self) -> Self {
        Dv::new(self.x.add(o.x), self.y.add(o.y))
    }
    pub(crate) fn sub(self, o: Self) -> Self {
        Dv::new(self.x.sub(o.x), self.y.sub(o.y))
    }
    pub(crate) fn scale(self, s: D) -> Self {
        Dv::new(self.x.mul(s), self.y.mul(s))
    }
    pub(crate) fn mid(self, o: Self) -> Self {
        self.add(o).scale(D::c(0.5))
    }
    pub(crate) fn dot(self, o: Self) -> D {
        self.x.mul(o.x).add(self.y.mul(o.y))
    }
    pub(crate) fn cross(self, o: Self) -> D {
        self.x.mul(o.y).sub(self.y.mul(o.x))
    }
    pub(crate) fn len(self) -> D {
        self.x.hypot(self.y)
    }
    pub(crate) fn dist(self, o: Self) -> D {
        self.sub(o).len()
    }
    pub(crate) fn perp(self) -> Self {
        Dv::new(self.y.neg(), self.x)
    }
    /// [`Vec2::normalized`]: zero (and a zero derivative) at or below [`cadcraft_geom::EPS`].
    pub(crate) fn normalized(self) -> Self {
        let l = self.len();
        if l.v > cadcraft_geom::EPS { self.scale(l.recip()) } else { Dv::c(Vec2::ZERO) }
    }
}

/// Unit direction `(b − a) / |b − a|`, or the raw difference when shorter than `1e-12`.
pub(crate) fn dir_n(a: Dv, b: Dv) -> Dv {
    let d = b.sub(a);
    let l = d.len();
    if l.v > 1e-12 { d.scale(l.recip()) } else { d }
}

pub(crate) fn ray(a: Dv, b: Dv, rev: bool) -> Dv {
    if rev { dir_n(b, a) } else { dir_n(a, b) }
}

pub(crate) fn wrap(a: D) -> D {
    let r = (a.v + PI).rem_euclid(TAU) - PI;
    if r.is_finite() { D { v: r, d: a.d } } else { D::c(0.0) }
}

/// Ellipse in dual parameters. Angles `t0`/`t1` are present for arcs.
pub(crate) struct EllD {
    pub c: Dv,
    pub mj: Dv,
    pub ratio: D,
    pub t0: D,
    pub t1: D,
}

impl EllD {
    pub(crate) fn minor(&self) -> Dv {
        self.mj.perp().scale(self.ratio)
    }
    fn axes(&self) -> (D, D) {
        let a = self.mj.len();
        (a, a.mul(self.ratio.abs()))
    }
    /// [`Ell::on`].
    pub(crate) fn on(&self, p: Dv) -> D {
        let (a, b) = self.axes();
        if crate::curves::not_above(a.v, 1e-300) || crate::curves::not_above(b.v, 1e-300) {
            return p.dist(self.c).sub(a);
        }
        let u_dir = self.mj.scale(a.recip());
        let v_dir = u_dir.perp();
        let q = p.sub(self.c);
        let u = q.dot(u_dir);
        let v = q.dot(v_dir);
        let s = a.div(b);
        let vs = v.mul(s);
        u.mul(u).add(vs.mul(vs)).sqrt().sub(a)
    }
    /// [`Ell::support`].
    pub(crate) fn support(&self, n: Dv) -> D {
        let x = n.dot(self.mj);
        let y = n.dot(self.minor());
        x.hypot(y)
    }
    pub(crate) fn at(&self, t: D) -> Dv {
        self.c.add(self.mj.scale(t.cos())).add(self.minor().scale(t.sin()))
    }
    pub(crate) fn d1(&self, t: D) -> Dv {
        self.mj.scale(t.sin().neg()).add(self.minor().scale(t.cos()))
    }
}

/// Geometry read at `x` with parameter `seed` differentiated.
pub(crate) struct Ctx<'a> {
    pub model: &'a Model,
    pub x: &'a [f64],
    pub seed: usize,
}

impl Ctx<'_> {
    pub(crate) fn num(&self, i: usize) -> Option<D> {
        let v = *self.x.get(i)?;
        Some(D { v, d: if i == self.seed { 1.0 } else { 0.0 } })
    }
    fn p2(&self, o: usize) -> Option<Dv> {
        Some(Dv::new(self.num(o)?, self.num(o + 1)?))
    }
    fn sweep(&self, a0: D, a1: D) -> D {
        let s = norm_angle(a1.v - a0.v);
        if s <= cadcraft_geom::EPS {
            D::c(TAU)
        } else {
            // `norm_angle` is a shift of `rem_euclid` (derivative 1) away from the cut.
            D { v: ccw_sweep(a0.v, a1.v), d: a1.d - a0.d }
        }
    }
    fn ell_at(&self, o: usize, arc: bool) -> Option<EllD> {
        let (t0, t1) = if arc { (self.num(o + 5)?, self.num(o + 6)?) } else { (D::c(0.0), D::c(TAU)) };
        Some(EllD { c: self.p2(o)?, mj: self.p2(o + 2)?, ratio: self.num(o + 4)?, t0, t1 })
    }
    pub(crate) fn point(&self, r: &GeomRef) -> Option<Dv> {
        let s = self.model.slot(r.handle)?;
        let o = s.off;
        match (s.shape, r.sub) {
            (Shape::Line, Sub::Start) => self.p2(o),
            (Shape::Line, Sub::End) => self.p2(o + 2),
            (Shape::Line, Sub::Mid) => Some(self.p2(o)?.mid(self.p2(o + 2)?)),
            (Shape::Circle | Shape::Arc, Sub::Center) => self.p2(o),
            (Shape::Arc, Sub::Start | Sub::End | Sub::Mid) => self.arc_point(o, r.sub),
            (Shape::Poly { n, .. }, Sub::Vertex(i)) if (i as usize) < n => self.p2(o + 2 * i as usize),
            (Shape::Poly { .. }, Sub::Start) => self.p2(o),
            (Shape::Poly { n, .. }, Sub::End) => self.p2(o + 2 * n.checked_sub(1)?),
            (Shape::Point, Sub::Whole | Sub::Start | Sub::Center) => self.p2(o),
            (Shape::Ellipse { .. }, Sub::Center) => self.p2(o),
            (Shape::Ellipse { .. }, Sub::Vertex(i)) => self.ellipse_vertex(o, i),
            (Shape::Ellipse { arc: true }, Sub::Start | Sub::End | Sub::Mid) => self.ellipse_end(o, r.sub),
            (Shape::Spline { n, .. }, Sub::Vertex(i)) if (i as usize) < n => self.p2(o + 2 * i as usize),
            (Shape::Spline { ends: true, .. }, Sub::Start) => self.p2(o),
            (Shape::Spline { n, ends: true, .. }, Sub::End) => self.p2(o + 2 * n.checked_sub(1)?),
            (Shape::Insert { .. }, Sub::Whole | Sub::Start | Sub::Center) => self.p2(o),
            (Shape::Insert { n }, Sub::Vertex(i)) if (i as usize) < n => self.insert_vertex(s.handle, o, i as usize),
            _ => None,
        }
    }
    fn arc_point(&self, o: usize, sub: Sub) -> Option<Dv> {
        let c = self.p2(o)?;
        let rad = self.num(o + 2)?;
        let a0 = self.num(o + 3)?;
        let a1 = self.num(o + 4)?;
        let sweep = self.sweep(a0, a1);
        let a = match sub {
            Sub::Start => a0,
            Sub::End => a1,
            _ => a0.add(sweep.scale_const(0.5)),
        };
        Some(c.add(Dv::new(a.cos(), a.sin()).scale(rad)))
    }
    fn ellipse_vertex(&self, o: usize, i: u32) -> Option<Dv> {
        let e = self.ell_at(o, false)?;
        Some(match i {
            0 => e.c.add(e.mj),
            1 => e.c.add(e.minor()),
            2 => e.c.sub(e.mj),
            3 => e.c.sub(e.minor()),
            _ => return None,
        })
    }
    fn ellipse_end(&self, o: usize, sub: Sub) -> Option<Dv> {
        let e = self.ell_at(o, true)?;
        let t = match sub {
            Sub::Start => e.t0,
            Sub::End => e.t1,
            _ => e.t0.add(self.sweep(e.t0, e.t1).scale_const(0.5)),
        };
        Some(e.at(t))
    }
    fn insert_vertex(&self, handle: cadcraft_doc::Handle, o: usize, i: usize) -> Option<Dv> {
        let q = *self.model.rigid_local(handle)?.get(i)?;
        let origin = self.p2(o)?;
        let theta = self.num(o + 2)?;
        Some(origin.add(rotate_const(q, theta)))
    }
    pub(crate) fn line(&self, r: &GeomRef) -> Option<(Dv, Dv)> {
        let s = self.model.slot(r.handle)?;
        let o = s.off;
        match (s.shape, r.sub) {
            (Shape::Line, Sub::Whole) => Some((self.p2(o)?, self.p2(o + 2)?)),
            (Shape::Poly { n, closed }, Sub::Segment(i)) => {
                let (a, b) = Model::seg(n, closed, i as usize)?;
                Some((self.p2(o + 2 * a)?, self.p2(o + 2 * b)?))
            }
            (Shape::Poly { n: 2, .. }, Sub::Whole) => Some((self.p2(o)?, self.p2(o + 2)?)),
            _ => None,
        }
    }
    pub(crate) fn circle(&self, r: &GeomRef) -> Option<(Dv, D)> {
        let s = self.model.slot(r.handle)?;
        match (s.shape, r.sub) {
            (Shape::Circle | Shape::Arc, Sub::Whole) => Some((self.p2(s.off)?, self.num(s.off + 2)?)),
            _ => None,
        }
    }
    pub(crate) fn ellipse(&self, r: &GeomRef) -> Option<EllD> {
        let s = self.model.slot(r.handle)?;
        match (s.shape, r.sub) {
            (Shape::Ellipse { arc }, Sub::Whole) => self.ell_at(s.off, arc),
            _ => None,
        }
    }
    pub(crate) fn arc_sweep(&self, r: &GeomRef) -> Option<D> {
        let s = self.model.slot(r.handle)?;
        match (s.shape, r.sub) {
            (Shape::Arc, Sub::Whole) => {
                let a0 = self.num(s.off + 3)?;
                let a1 = self.num(s.off + 4)?;
                Some(self.sweep(a0, a1))
            }
            _ => None,
        }
    }
    /// Major-axis segment `(c − mj, c + mj)`, used when a line residual accepts an ellipse.
    pub(crate) fn ellipse_axis(&self, r: &GeomRef) -> Option<(Dv, Dv)> {
        let e = self.ellipse(r)?;
        Some((e.c.sub(e.mj), e.c.add(e.mj)))
    }
}

impl D {
    fn scale_const(self, s: f64) -> Self {
        D { v: self.v * s, d: self.d * s }
    }
}

fn norm_angle(a: f64) -> f64 {
    if !a.is_finite() {
        return 0.0;
    }
    let r = a.rem_euclid(TAU);
    if r >= TAU { 0.0 } else { r }
}

fn rotate_const(q: Vec2, theta: D) -> Dv {
    let (s, c) = theta.v.sin_cos();
    let rx = q.x * c - q.y * s;
    let ry = q.x * s + q.y * c;
    Dv::new(D { v: rx, d: -ry * theta.d }, D { v: ry, d: rx * theta.d })
}

/// Circle point and derivative `(c + r (cos t, sin t), r (−sin t, cos t))`.
pub(crate) fn circle_at(c: Dv, rad: D, t: D) -> (Dv, Dv) {
    let (co, si) = (t.cos(), t.sin());
    let p = c.add(Dv::new(co, si).scale(rad));
    let d1 = Dv::new(si.neg(), co).scale(rad);
    (p, d1)
}
