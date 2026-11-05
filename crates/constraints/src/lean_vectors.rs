//! Checks cadcraft's rank / DOF routine and the ordered-versus-dense normal solve against the
//! exact Lean vectors in `lean/solver/tests/vectors/dof.csv` (ticket #219).
//!
//! The CSV is produced by `lean/solver/Generate.lean`: exact rational Jacobians, their exact rank
//! (`CraftSolver.gsRank`, proved equal to `Matrix.rank`), DOF and redundancy. `source` is
//! `sketch` for Jacobians exported from real cadcraft sketches (rebuilt here and compared entry by
//! entry), `synthetic` for hand-built ones, `tolerance` for a matrix with a tiny but real column (the
//! finite-difference noise cut would drop it; the analytic path must not).

use std::collections::HashMap;

use cadcraft_doc::{
    Common, Constraint, ConstraintKind as K, DistAxis, Drawing, EntityKind, GeomRef, Handle, Line as DLine, Point as DPoint, Space, Sub,
};
use cadcraft_geom::Vec3;

use crate::{SolveOptions, build, dof, jac, lm, sparse};

const CSV: &str = include_str!("../../../lean/solver/tests/vectors/dof.csv");

struct Row {
    case: String,
    source: String,
    rows: usize,
    cols: usize,
    m: Vec<f64>,
    rank: usize,
    dof: usize,
    redundant: usize,
}

fn rational(s: &str) -> f64 {
    match s.split_once('/') {
        Some((p, q)) => p.parse::<f64>().unwrap() / q.parse::<f64>().unwrap(),
        None => s.parse::<f64>().unwrap(),
    }
}

fn vectors() -> Vec<Row> {
    let mut lines = CSV.lines();
    assert_eq!(lines.next(), Some("case,source,rows,cols,matrix,rank,dof,redundant"));
    lines
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            let (rows, cols) = (f[2].parse().unwrap(), f[3].parse().unwrap());
            let m: Vec<f64> = f[4].split(';').flat_map(|r| r.split(' ').filter(|e| !e.is_empty()).map(rational).collect::<Vec<_>>()).collect();
            assert_eq!(m.len(), rows * cols, "{}", f[0]);
            Row {
                case: f[0].into(),
                source: f[1].into(),
                rows,
                cols,
                m,
                rank: f[5].parse().unwrap(),
                dof: f[6].parse().unwrap(),
                redundant: f[7].parse().unwrap(),
            }
        })
        .collect()
}

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }
    fn perm(&mut self, n: usize) -> Vec<usize> {
        let mut p: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            p.swap(i, (self.next() as usize) % (i + 1));
        }
        p
    }
}

// ---- sketches (geometry chosen so every Jacobian entry is rational) ----

fn put(d: &mut Drawing, k: EntityKind) -> Handle {
    d.add(&Space::Model, Common::default(), k).unwrap()
}
fn point(d: &mut Drawing, x: f64, y: f64) -> Handle {
    put(d, EntityKind::Point(DPoint { p: Vec3::new(x, y, 0.0), angle: 0.0 }))
}
fn line(d: &mut Drawing, a: (f64, f64), b: (f64, f64)) -> Handle {
    put(d, EntityKind::Line(DLine { a: Vec3::new(a.0, a.1, 0.0), b: Vec3::new(b.0, b.1, 0.0) }))
}
fn w(h: Handle) -> GeomRef {
    GeomRef::whole(h)
}
fn s(h: Handle, sub: Sub) -> GeomRef {
    GeomRef::new(h, sub)
}
fn push(d: &mut Drawing, kind: K, refs: &[GeomRef], expr: &str) {
    let id = d.constraints.len() as u32 + 1;
    d.constraints.push(Constraint { id, kind, refs: refs.to_vec(), name: String::new(), expr: expr.into() });
}
fn aligned() -> K {
    K::Distance(DistAxis::Aligned)
}

fn rectangle(fixed: bool, extra: bool) -> Drawing {
    let mut d = Drawing::default();
    let l = [
        line(&mut d, (0.0, 0.0), (12.0, 0.0)),
        line(&mut d, (12.0, 0.0), (12.0, 4.0)),
        line(&mut d, (12.0, 4.0), (0.0, 4.0)),
        line(&mut d, (0.0, 4.0), (0.0, 0.0)),
    ];
    for i in 0..4 {
        push(&mut d, K::Coincident, &[s(l[i], Sub::End), s(l[(i + 1) % 4], Sub::Start)], "");
    }
    push(&mut d, K::Horizontal, &[w(l[0])], "");
    push(&mut d, K::Horizontal, &[w(l[2])], "");
    push(&mut d, K::Vertical, &[w(l[1])], "");
    push(&mut d, K::Vertical, &[w(l[3])], "");
    if fixed {
        push(&mut d, K::Fix, &[s(l[0], Sub::Start)], "");
        push(&mut d, aligned(), &[w(l[0])], "12");
        push(&mut d, aligned(), &[w(l[1])], "4");
    }
    if extra {
        push(&mut d, aligned(), &[w(l[3])], "4");
    }
    d
}

fn triangle(fix: bool) -> Drawing {
    let mut d = Drawing::default();
    let p = [point(&mut d, 0.0, 0.0), point(&mut d, 4.0, 0.0), point(&mut d, 4.0, 3.0)];
    push(&mut d, aligned(), &[w(p[0]), w(p[1])], "4");
    push(&mut d, aligned(), &[w(p[1]), w(p[2])], "3");
    push(&mut d, aligned(), &[w(p[2]), w(p[0])], "5");
    if fix {
        push(&mut d, K::Fix, &[w(p[0])], "");
        push(&mut d, K::Horizontal, &[w(p[0]), w(p[1])], "");
    }
    d
}

/// Seven points in a chain, each step (3,4): aligned distance 5.
fn chain() -> Drawing {
    let mut d = Drawing::default();
    let p: Vec<Handle> = (0..7).map(|i| point(&mut d, 3.0 * i as f64, 4.0 * i as f64)).collect();
    for i in 0..6 {
        push(&mut d, aligned(), &[w(p[i]), w(p[i + 1])], "5");
    }
    d
}

fn parallel_perpendicular() -> Drawing {
    let mut d = Drawing::default();
    let a = line(&mut d, (0.0, 0.0), (4.0, 0.0));
    let b = line(&mut d, (0.0, 2.0), (4.0, 2.0));
    let c = line(&mut d, (4.0, 0.0), (4.0, 2.0));
    push(&mut d, K::Parallel, &[w(a), w(b)], "");
    push(&mut d, K::Perpendicular, &[w(a), w(c)], "");
    push(&mut d, K::Equal, &[w(a), w(b)], "");
    d
}

fn sketches() -> Vec<(&'static str, Drawing)> {
    vec![
        ("rect_open", rectangle(false, false)),
        ("rect_fixed", rectangle(true, false)),
        ("rect_redundant", rectangle(true, true)),
        ("triangle_345", triangle(false)),
        ("triangle_345_fixed", triangle(true)),
        ("chain7_dist", chain()),
        ("parallel_perp_equal", parallel_perpendicular()),
    ]
}

/// Analytic Jacobian of the sketch's single component: (rows, cols, row-major).
fn jacobian(d: &Drawing) -> (usize, usize, Vec<f64>) {
    let cons: Vec<&Constraint> = d.constraints.iter().collect();
    let (comps, skip) = build(d, &cons, &SolveOptions::default()).unwrap();
    assert!(skip.is_empty(), "{skip:?}");
    assert_eq!(comps.len(), 1);
    let comp = &comps[0];
    let built: Vec<&crate::residual::Built> = comp.built.iter().collect();
    let free = comp.free(None);
    let m = jac::dense(&comp.model, &built, &comp.model.x0, &free);
    (comp.rows(None), free.len(), m)
}

/// Best rational `p/q` (q <= 1000) for `x`, or `None` when none is within 1e-12.
fn approx(x: f64) -> Option<(i64, i64)> {
    (1..=1000i64).find_map(|q| {
        let p = (x * q as f64).round();
        ((p / q as f64 - x).abs() < 1e-12).then_some((p as i64, q))
    })
}

fn fmt(x: f64) -> String {
    let (p, q) = approx(x).unwrap_or_else(|| panic!("{x} is not a small rational"));
    let g = gcd(p.abs(), q);
    let (p, q) = (p / g, q / g);
    if q == 1 { p.to_string() } else { format!("{p}/{q}") }
}
fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 { a.max(1) } else { gcd(b, a % b) }
}

/// Prints the Lean literal of every sketch Jacobian (`Generate.lean` embeds it). Run with
/// `cargo test -p cadcraft-constraints lean_vectors::export -- --ignored --nocapture`.
#[test]
#[ignore]
fn export() {
    for (name, d) in sketches() {
        let (rows, cols, m) = jacobian(&d);
        println!("  (\"{name}\", \"sketch\", [");
        for r in 0..rows {
            let row: Vec<String> = (0..cols).map(|c| fmt(m[r * cols + c])).collect();
            println!("    [{}],", row.iter().map(|e| format!("({e} : ℚ)")).collect::<Vec<_>>().join(", "));
        }
        println!("  ]),");
    }
}

// ---- tests ----

#[test]
fn rank_and_dof_match_exact_lean_values() {
    let v = vectors();
    assert!(v.len() >= 20, "{} vectors", v.len());
    for r in &v {
        let rank = lm::rank_qr(&r.m, r.rows, r.cols, lm::ANALYTIC_NOISE);
        assert_eq!(r.dof, r.cols - r.rank, "{}: csv dof", r.case);
        assert_eq!(r.redundant, r.rows - r.rank, "{}: csv redundancy", r.case);
        assert_eq!(rank, r.rank, "{}", r.case);
        assert_eq!(r.cols - rank.min(r.cols), r.dof, "{}", r.case);
        if r.source == "tolerance" {
            // The finite-difference noise cut is not rank-preserving; it must not apply to analytic Jacobians.
            let fd = lm::rank_qr(&r.m, r.rows, r.cols, lm::COL_NOISE);
            assert!(fd < r.rank, "{}: the finite-difference cut should lose exact rank {} (got {fd})", r.case, r.rank);
        }
    }
}

#[test]
fn rank_is_invariant_under_permutation_and_scaling() {
    let mut rng = Lcg(0x219);
    for r in vectors().iter() {
        let rp = rng.perm(r.rows);
        let cp = rng.perm(r.cols);
        let rs: Vec<f64> = (0..r.rows).map(|_| 10f64.powi((rng.next() % 5) as i32 - 2)).collect();
        let cs: Vec<f64> = (0..r.cols).map(|_| 10f64.powi((rng.next() % 5) as i32 - 2)).collect();
        let mut m = vec![0.0; r.m.len()];
        for i in 0..r.rows {
            for j in 0..r.cols {
                m[i * r.cols + j] = r.m[rp[i] * r.cols + cp[j]];
            }
        }
        assert_eq!(lm::rank_qr(&m, r.rows, r.cols, lm::ANALYTIC_NOISE), r.rank, "{}: permuted", r.case);
        for i in 0..r.rows {
            for j in 0..r.cols {
                m[i * r.cols + j] *= rs[i] * cs[j];
            }
        }
        assert_eq!(lm::rank_qr(&m, r.rows, r.cols, lm::ANALYTIC_NOISE), r.rank, "{}: permuted and scaled", r.case);
    }
}

#[test]
fn sketch_jacobians_and_dof_match_cadcraft() {
    let v: HashMap<String, Row> = vectors().into_iter().map(|r| (r.case.clone(), r)).collect();
    let mut seen = 0;
    for (name, d) in sketches() {
        let r = v.get(name).unwrap_or_else(|| panic!("no vector for {name}"));
        assert_eq!(r.source, "sketch");
        let (rows, cols, m) = jacobian(&d);
        assert_eq!((rows, cols), (r.rows, r.cols), "{name}");
        for (k, (a, b)) in m.iter().zip(&r.m).enumerate() {
            assert!((a - b).abs() < 1e-12, "{name} entry {k}: cadcraft {a}, lean {b}");
        }
        let report = dof(&d, &[]).unwrap();
        assert_eq!(report.dof, Some(r.dof), "{name}: {report:?}");
        assert_eq!(report.redundant, r.redundant, "{name}: {report:?}");
        seen += 1;
    }
    assert_eq!(seen, v.values().filter(|r| r.source == "sketch").count());
}

#[test]
fn ordered_banded_solve_matches_dense_on_every_vector() {
    let mut rng = Lcg(0x5eed);
    let mut worst = 0.0f64;
    let mut checked = 0;
    for r in vectors().iter().filter(|r| r.m.iter().any(|v| *v != 0.0)) {
        let winv: Vec<f64> = (0..r.cols).map(|_| 0.2 + (rng.next() % 11) as f64 * 0.3).collect();
        let rhs: Vec<f64> = (0..r.rows).map(|i| (i as f64 * 0.37).sin()).collect();
        // Scramble the row order so the RCM permutation is non-trivial; compare back in place.
        let scramble = rng.perm(r.rows);
        let mut trips = Vec::new();
        for (i, &si) in scramble.iter().enumerate() {
            for j in 0..r.cols {
                let x = r.m[i * r.cols + j];
                if x != 0.0 {
                    trips.push((si as u32, j as u32, x));
                }
            }
        }
        let mut jr = vec![0.0; r.m.len()];
        for (i, &si) in scramble.iter().enumerate() {
            jr[si * r.cols..(si + 1) * r.cols].copy_from_slice(&r.m[i * r.cols..(i + 1) * r.cols]);
        }
        for mu in [1e-4, 1e-8] {
            let dense = lm::dense_normal(&jr, &winv, &rhs, r.rows, r.cols, mu).unwrap();
            let Ok(sp) = sparse::solve_damped(r.rows, &trips, &winv, &rhs, mu) else { continue };
            let scale = dense.iter().fold(1.0f64, |a, v| a.max(v.abs()));
            let delta = dense.iter().zip(&sp).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max) / scale;
            // Full row rank: the damped normal matrix is well conditioned and both orderings agree
            // to roundoff. Redundant rows (rank < rows) leave eigenvalues of order `mu * diag`, so
            // the condition number is about 1e8 at `mu = 1e-8` and the two orderings differ by
            // up to about 1e-8 relative (measured worst 6.4e-9).
            let limit = if r.rank == r.rows { 1e-10 } else { 1e-7 };
            assert!(delta <= limit, "{} mu {mu:e}: relative difference {delta:e} > {limit:e}", r.case);
            if delta > 1e-10 {
                eprintln!("  {} mu {mu:e}: relative difference {delta:e} (rank {} of {} rows)", r.case, r.rank, r.rows);
            }
            worst = worst.max(delta);
            checked += 1;
        }
    }
    eprintln!("ordered vs dense: {checked} solves, worst relative difference {worst:e}");
    assert!(checked >= 20);
}
