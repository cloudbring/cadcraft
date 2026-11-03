//! Synthetic chain timings. Ignored: the dense "before" factor is cubic in the row count.

use crate::lm;
use crate::{SolveOptions, build};
use cadcraft_doc::{Common, Constraint, ConstraintKind, DistAxis, Drawing, EntityKind, GeomRef, Point as DPoint, Space};
use cadcraft_geom::Vec3;

fn chain(rows: usize) -> Drawing {
    let mut d = Drawing::default();
    let mut prev = None;
    assert_eq!(rows % 2, 0);
    for i in 0..=rows / 2 {
        let y = 0.05 * (i as f64 * 0.37).sin();
        let h =
            d.add(&Space::Model, Common::default(), EntityKind::Point(DPoint { p: Vec3::new(i as f64 * 2.0, y, 0.0), angle: 0.0 })).expect("point");
        if let Some(p) = prev {
            for (offset, axis, target) in [(0, DistAxis::Horizontal, "2"), (1, DistAxis::Vertical, "0")] {
                d.constraints.push(Constraint {
                    id: (2 * i + offset) as u32,
                    kind: ConstraintKind::Distance(axis),
                    refs: vec![GeomRef::whole(p), GeomRef::whole(h)],
                    name: String::new(),
                    expr: target.into(),
                });
            }
        }
        prev = Some(h);
    }
    d
}

fn time_ms(rows: usize, dense_fd: bool) -> (f64, bool) {
    let d = chain(rows);
    let cons: Vec<&Constraint> = d.constraints.iter().collect();
    let (comps, skip) = build(&d, &cons, &SolveOptions::default()).expect("build");
    assert!(skip.is_empty(), "{skip:?}");
    let comp = &comps[0];
    let free = comp.free(None);
    let f = |x: &[f64], out: &mut Vec<f64>| comp.eval(x, out, None);
    assert_eq!(comp.rows(None), rows);
    let start = std::time::Instant::now();
    let o = if dense_fd { lm::solve_dense_legacy(&f, &comp.model.x0, &free, &comp.weights, comp.tol, 40) } else { comp.run(None).expect("solve") };
    (start.elapsed().as_secs_f64() * 1e3, o.converged)
}

#[test]
fn chain_past_the_old_row_cap_converges() {
    let (ms, ok) = time_ms(802, false);
    assert!(ok, "sparse chain did not converge ({ms:.2} ms)");
}

#[test]
fn sparse_chain_matches_dense_above_dispatch_threshold() {
    let drawing = chain(80);
    let constraints: Vec<&Constraint> = drawing.constraints.iter().collect();
    let (components, skipped) = build(&drawing, &constraints, &SolveOptions::default()).unwrap();
    assert!(skipped.is_empty());
    let dense = lm::solve_fixture(&components[0], false);
    let sparse = components[0].run(None).unwrap();
    assert!(dense.converged && sparse.converged);
    let delta = dense.x.iter().zip(&sparse.x).map(|(dense, sparse)| (dense - sparse).abs()).fold(0.0, f64::max);
    assert!(delta <= 1e-9, "sparse/dense chain delta {delta:e}");
    eprintln!("80-row chain sparse/dense delta {delta:e}");
}

#[test]
fn nonlinear_sparse_chain_matches_dense_above_dispatch_threshold() {
    let mut drawing = chain(160);
    drawing.constraints.retain(|constraint| constraint.kind == ConstraintKind::Distance(DistAxis::Horizontal));
    for constraint in &mut drawing.constraints {
        constraint.kind = ConstraintKind::Distance(DistAxis::Aligned);
    }
    let constraints: Vec<&Constraint> = drawing.constraints.iter().collect();
    let (components, skipped) = build(&drawing, &constraints, &SolveOptions::default()).unwrap();
    assert!(skipped.is_empty());
    let dense = lm::solve_fixture(&components[0], false);
    let sparse = components[0].run(None).unwrap();
    assert!(dense.converged && sparse.converged);
    let delta = dense.x.iter().zip(&sparse.x).map(|(dense, sparse)| (dense - sparse).abs()).fold(0.0, f64::max);
    assert!(delta <= 1e-9, "sparse/dense nonlinear chain delta {delta:e}");
    eprintln!("80-row nonlinear chain sparse/dense delta {delta:e}");
}

/// Milliseconds per solve of a point chain at 200, 800 and 2,000 residual rows.
/// Before: central-difference Jacobian and dense Gaussian elimination.
/// After: analytic Jacobian and envelope Cholesky.
#[test]
#[ignore = "times the cubic dense factor; run with --ignored --release"]
fn synthetic_chain_before_after_ms() {
    for rows in [200, 800, 2_000] {
        let (before, ok_b) = time_ms(rows, true);
        let (after, ok_a) = time_ms(rows, false);
        assert!(ok_b && ok_a, "rows {rows} before {before:.1} ({ok_b}) after {after:.1} ({ok_a})");
        eprintln!("rows {rows}: dense-fd {before:.3} ms, analytic-sparse {after:.3} ms");
    }
}
