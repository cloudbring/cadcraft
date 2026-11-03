//! Exact sign tests for orientation and the incircle predicate.
//!
//! Runtime kernel: the `robust` crate, a pure-Rust port of Jonathan Richard Shewchuk's
//! adaptive-precision predicates ("Adaptive Precision Floating-Point Arithmetic and Fast Robust
//! Geometric Predicates", Discrete & Computational Geometry 18(3), 1997). The returned sign is
//! exact for finite coordinates inside Shewchuk's exponent range (about `1e-142` to `1e201`).
//! A later proof target is Attene's indirect integer predicates; Shewchuk expansions have no
//! machine-checked proof (artcraft-algorithms #4, #13).
//!
//! Non-finite input does not panic. It is reported as collinear / cocircular (`Equal`), and the
//! determinant helpers return `NaN`.

use std::cmp::Ordering;

use crate::Vec2;

/// Which way `a → b → c` turns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    /// Clockwise (negative orientation).
    Cw,
    /// Counter-clockwise (positive orientation).
    Ccw,
    /// The three points are collinear, or an input was not finite.
    Collinear,
}

impl Orientation {
    /// Swap the sign. Collinear stays collinear.
    pub fn reversed(self) -> Self {
        match self {
            Orientation::Cw => Orientation::Ccw,
            Orientation::Ccw => Orientation::Cw,
            Orientation::Collinear => Orientation::Collinear,
        }
    }
}

fn coord(p: Vec2) -> robust::Coord<f64> {
    robust::Coord { x: p.x, y: p.y }
}

fn finite3(a: Vec2, b: Vec2, c: Vec2) -> bool {
    a.is_finite() && b.is_finite() && c.is_finite()
}

/// Signed `orient2d(a, b, c)`: positive when `c` is left of directed line `a → b`.
/// `NaN` when an input is not finite. Zero exactly when the points are collinear.
pub(crate) fn orient_det(a: Vec2, b: Vec2, c: Vec2) -> f64 {
    if !finite3(a, b, c) {
        return f64::NAN;
    }
    robust::orient2d(coord(a), coord(b), coord(c))
}

/// Sign of [`orient_det`].
pub fn orient2d(a: Vec2, b: Vec2, c: Vec2) -> Orientation {
    let d = orient_det(a, b, c);
    if d > 0.0 {
        Orientation::Ccw
    } else if d < 0.0 {
        Orientation::Cw
    } else {
        Orientation::Collinear
    }
}

/// Signed incircle determinant. Positive when `d` is inside the circle through `a`, `b`, `c`
/// and those three are counter-clockwise. The sign flips when `a`, `b`, `c` are clockwise.
/// `NaN` when an input is not finite.
pub(crate) fn incircle_det(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> f64 {
    if !(finite3(a, b, c) && d.is_finite()) {
        return f64::NAN;
    }
    robust::incircle(coord(a), coord(b), coord(c), coord(d))
}

/// Sign of [`incircle_det`]. `Greater` is inside a counter-clockwise circle, `Less` is outside,
/// `Equal` is cocircular or a non-finite input.
pub fn incircle(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> Ordering {
    let det = incircle_det(a, b, c, d);
    if det > 0.0 {
        Ordering::Greater
    } else if det < 0.0 {
        Ordering::Less
    } else {
        Ordering::Equal
    }
}

/// `p` lies on the closed segment `a`—`b`, decided by [`orient2d`] and the bounding box.
/// A zero-length segment contains only its own point.
pub fn point_on_segment(p: Vec2, a: Vec2, b: Vec2) -> bool {
    if orient2d(a, b, p) != Orientation::Collinear {
        return false;
    }
    let (min_x, max_x) = if a.x <= b.x { (a.x, b.x) } else { (b.x, a.x) };
    let (min_y, max_y) = if a.y <= b.y { (a.y, b.y) } else { (b.y, a.y) };
    p.x >= min_x && p.x <= max_x && p.y >= min_y && p.y <= max_y
}

/// Whether closed segments `a`—`b` and `c`—`d` meet, including an endpoint touch and a
/// collinear overlap. Parallel segments that only meet when extended do not.
pub fn segment_intersects(a: Vec2, b: Vec2, c: Vec2, d: Vec2) -> bool {
    let ab_c = orient2d(a, b, c);
    let ab_d = orient2d(a, b, d);
    let cd_a = orient2d(c, d, a);
    let cd_b = orient2d(c, d, b);
    if ab_c != ab_d && cd_a != cd_b {
        return true;
    }
    (ab_c == Orientation::Collinear && point_on_segment(c, a, b))
        || (ab_d == Orientation::Collinear && point_on_segment(d, a, b))
        || (cd_a == Orientation::Collinear && point_on_segment(a, c, d))
        || (cd_b == Orientation::Collinear && point_on_segment(b, c, d))
}

/// The open segment `a`—`b` crosses the ray that starts at `p` and runs toward +X.
/// A point that lands on the segment is not a crossing, and a vertex is charged to the
/// endpoint strictly above `p` (the usual even-odd rule).
pub(crate) fn ray_crosses_right(a: Vec2, b: Vec2, p: Vec2) -> bool {
    if (a.y > p.y) == (b.y > p.y) {
        return false;
    }
    match orient2d(a, b, p) {
        Orientation::Ccw => b.y > a.y,
        Orientation::Cw => a.y > b.y,
        Orientation::Collinear => false,
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use proptest::prelude::*;

    use super::{Orientation, incircle, orient2d, point_on_segment, segment_intersects};
    use crate::Vec2;

    fn naive_cross(a: Vec2, b: Vec2, c: Vec2) -> f64 {
        (b - a).cross(c - a)
    }

    #[test]
    fn orient2d_classifies_a_unit_triangle() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(1.0, 0.0);
        let c = Vec2::new(0.0, 1.0);
        assert_eq!(orient2d(a, b, c), Orientation::Ccw);
        assert_eq!(orient2d(a, c, b), Orientation::Cw);
        assert_eq!(orient2d(a, b, Vec2::new(2.0, 0.0)), Orientation::Collinear);
        assert_eq!(orient2d(a, b, c).reversed(), Orientation::Cw);
    }

    #[test]
    fn orient2d_non_finite_is_collinear_and_does_not_panic() {
        assert_eq!(orient2d(Vec2::new(f64::NAN, 0.0), Vec2::ZERO, Vec2::X), Orientation::Collinear);
        assert_eq!(orient2d(Vec2::new(f64::INFINITY, 0.0), Vec2::ZERO, Vec2::X), Orientation::Collinear);
    }

    /// Points at a 1e8–1e9 offset whose exact orientation is +1. The cross product this crate
    /// used to trust cancels to 0; the predicate does not.
    #[test]
    fn orient2d_large_offset_near_collinear_keeps_the_sign() {
        for (origin, span) in [(0.0, 1.0e8), (1.0e8, 1.0e8), (1.0e9, 1.0e8)] {
            let a = Vec2::new(origin, origin);
            let b = Vec2::new(origin + span, origin + span + 1.0);
            let c = Vec2::new(origin + span - 1.0, origin + span);
            assert_eq!(naive_cross(a, b, c), 0.0, "naive cross should cancel at {origin}");
            assert_eq!(orient2d(a, b, c), Orientation::Ccw, "origin {origin}");
            assert_eq!(orient2d(a, c, b), Orientation::Cw, "origin {origin}");
        }
    }

    /// The `0.5 + k·2^-53` grid. The two products in the fast filter agree to every stored bit;
    /// the exact determinant is `+2^-106`.
    #[test]
    fn orient2d_half_plus_ulp_grid_is_counter_clockwise() {
        let ulp = 2f64.powi(-53);
        let m = 2f64.powi(50);
        let a = Vec2::new(0.5, 0.5);
        let b = Vec2::new(0.5 + m * ulp, 0.5 + (m + 1.0) * ulp);
        let c = Vec2::new(0.5 + (m - 1.0) * ulp, 0.5 + m * ulp);
        assert_eq!(naive_cross(a, b, c), 0.0);
        assert_eq!(orient2d(a, b, c), Orientation::Ccw);
        // The same three points shifted onto a collinear grid stay collinear.
        let d = Vec2::new(0.5 + (m + 1.0) * ulp, 0.5 + (m + 1.0) * ulp);
        let e = Vec2::new(0.5 + (m + 2.0) * ulp, 0.5 + (m + 2.0) * ulp);
        assert_eq!(orient2d(a, d, e), Orientation::Collinear);
    }

    #[test]
    fn incircle_inside_outside_and_cocircular() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(1.0, 0.0);
        let c = Vec2::new(0.0, 1.0);
        assert_eq!(orient2d(a, b, c), Orientation::Ccw);
        assert_eq!(incircle(a, b, c, Vec2::new(0.2, 0.2)), Ordering::Greater);
        assert_eq!(incircle(a, b, c, Vec2::new(2.0, 2.0)), Ordering::Less);
        assert_eq!(incircle(a, b, c, Vec2::new(1.0, 1.0)), Ordering::Equal);
        // Clockwise circle: the inside point changes sign.
        assert_eq!(incircle(a, c, b, Vec2::new(0.2, 0.2)), Ordering::Less);
        assert_eq!(incircle(a, b, c, Vec2::new(f64::NAN, 0.0)), Ordering::Equal);
    }

    /// A cocircular case that an inexact incircle got backwards (robust crate issue 48;
    /// the sign is the one `predicates.c` reports).
    #[test]
    fn incircle_sign_on_shewchuk_near_cocircular_points() {
        let a = Vec2::new(2.1045541600524288e-15, -1.0000000000000016);
        let b = Vec2::new(1.000000000000005, -3.350874324301223e-16);
        let c = Vec2::new(7.553997323229233e-15, 0.9999999999999958);
        let d = Vec2::new(-0.9999999999999922, -7.073397829693697e-15);
        assert_eq!(incircle(a, b, c, d), Ordering::Less);
    }

    #[test]
    fn point_on_segment_includes_endpoints_only() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(4.0, 2.0);
        assert!(point_on_segment(a, a, b));
        assert!(point_on_segment(b, a, b));
        assert!(point_on_segment(Vec2::new(2.0, 1.0), a, b));
        assert!(!point_on_segment(Vec2::new(6.0, 3.0), a, b));
        assert!(!point_on_segment(Vec2::new(2.0, 1.1), a, b));
        assert!(point_on_segment(Vec2::new(0.0, 1.0), Vec2::new(0.0, 0.0), Vec2::new(0.0, 2.0)));
        assert!(!point_on_segment(Vec2::new(1.0, 0.0), a, a));
        assert!(point_on_segment(a, a, a));
    }

    #[test]
    fn segment_intersects_proper_touch_overlap_and_miss() {
        assert!(segment_intersects(Vec2::new(0.0, 0.0), Vec2::new(2.0, 2.0), Vec2::new(0.0, 2.0), Vec2::new(2.0, 0.0)));
        assert!(segment_intersects(Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0)));
        assert!(segment_intersects(Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(3.0, 0.0)));
        assert!(!segment_intersects(Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(3.0, 0.0)));
        assert!(!segment_intersects(Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(0.0, 1.0), Vec2::new(1.0, 1.0)));
        assert!(!segment_intersects(Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(0.5, 0.5), Vec2::new(0.5, 1.0)));
    }

    proptest! {
        #[test]
        fn orient2d_is_antisymmetric_and_cyclic(
            ax in -1e9f64..1e9,
            ay in -1e9f64..1e9,
            bx in -1e9f64..1e9,
            by in -1e9f64..1e9,
            cx in -1e9f64..1e9,
            cy in -1e9f64..1e9,
        ) {
            let a = Vec2::new(ax, ay);
            let b = Vec2::new(bx, by);
            let c = Vec2::new(cx, cy);
            let o = orient2d(a, b, c);
            prop_assert_eq!(orient2d(b, a, c), o.reversed());
            prop_assert_eq!(orient2d(a, c, b), o.reversed());
            prop_assert_eq!(orient2d(b, c, a), o);
            prop_assert_eq!(orient2d(c, a, b), o);
        }
    }
}
