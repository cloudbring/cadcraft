//! Synthetic chain timings. Ignored: the dense "before" factor is cubic in the row count.

use crate::lm::{self, Diff};
use crate::{SolveOptions, build};
use cadcraft_doc::{Common, Constraint, ConstraintKind, DistAxis, Drawing, EntityKind, GeomRef, Point as DPoint, Space};
use cadcraft_geom::Vec3;

fn chain(rows: usize) -> Drawing {
    let mut d = Drawing::default();
    let mut prev = None;
    for i in 0..=rows {
        let y = 0.05 * (i as f64 * 0.37).sin();
        let h =
            d.add(&Space::Model, Common::default(), EntityKind::Point(DPoint { p: Vec3::new(i as f64 * 2.0, y, 0.0), angle: 0.0 })).expect("point");
        if let Some(p) = prev {
            d.constraints.push(Constraint {
                id: i as u32,
                kind: ConstraintKind::Distance(DistAxis::Aligned),
                refs: vec![GeomRef::whole(p), GeomRef::whole(h)],
                name: String::new(),
                expr: "2".into(),
            });
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
    let built: Vec<&crate::residual::Built> = comp.built.iter().collect();
    let start = std::time::Instant::now();
    let o = if dense_fd {
        lm::solve_dense_legacy(&f, &comp.model.x0, &free, &comp.weights, comp.tol, 40)
    } else {
        lm::solve_diff(&f, &comp.model.x0, &free, &comp.weights, comp.tol, 40, &Diff::Analytic { model: &comp.model, built: &built })
    };
    (start.elapsed().as_secs_f64() * 1e3, o.converged)
}

#[test]
fn chain_past_the_old_row_cap_converges() {
    let (ms, ok) = time_ms(60, false);
    assert!(ok, "sparse chain did not converge ({ms:.2} ms)");
    assert!(ms < 5_000.0, "{ms:.2} ms");
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
        eprintln!("rows {rows}: dense-fd {before:.1} ms, analytic-sparse {after:.1} ms");
    }
}
