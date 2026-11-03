//! Degrees of freedom: how much the constraints leave the geometry free to move.
//!
//! Per connected component, the remaining DOF is the number of free parameters minus the rank
//! of the analytic constraint Jacobian at the current geometry (central differences only for
//! spline end frames); equations beyond the rank are redundant (over-constraint). A finite
//! difference at a point sitting on a centre is identically zero and used to call that
//! radial motion redundant. Per object, the DOF is the dimension of the null-space motion
//! restricted to the object's own parameters: how many independent ways the object can still
//! move while every constraint keeps holding (the rest of its component moving along as needed).

use std::collections::HashSet;

use cadcraft_doc::{Constraint, Drawing, Handle};
use serde::Serialize;

use crate::model::Model;
use crate::{Comp, MAX_CONSTRAINTS, SolveError, SolveOptions, build_budgeted, lm};

/// Most free parameters for which per-object freedom is analysed (null-space basis size).
const MAX_NULL_COLS: usize = 600;
/// Largest Jacobian (rows × columns × min) whose rank is computed.
const MAX_RANK_WORK: f64 = 4e8;
/// Largest residual-evaluation work (rows × free × per-residual cost, including spline
/// reconstruction) allowed before building the Jacobian during numerical differentiation.
const MAX_EVAL_WORK: f64 = 2e9;
/// Most objects listed in one report.
const MAX_OBJECTS: usize = 10_000;

/// Constraint state of an object or a selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DofStatus {
    /// No constraints.
    Unconstrained,
    /// Some freedom left.
    Under,
    /// No freedom left, no redundant constraints.
    Fully,
    /// Redundant constraints.
    Over,
    /// Too large to analyse.
    Unknown,
}

impl DofStatus {
    pub fn label(self) -> &'static str {
        match self {
            DofStatus::Unconstrained => "Not constrained",
            DofStatus::Under => "Under-constrained",
            DofStatus::Fully => "Fully constrained",
            DofStatus::Over => "Over-constrained",
            DofStatus::Unknown => "Unknown",
        }
    }
}

/// Degrees of freedom of one object.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectDof {
    pub handle: String,
    #[serde(rename = "type")]
    pub kind: &'static str,
    /// Parameters of the object: its degrees of freedom without constraints.
    pub params: usize,
    /// Remaining degrees of freedom (`None` when the system is too large to analyse).
    pub dof: Option<usize>,
    pub status: DofStatus,
}

/// Degrees of freedom of the drawing's constrained geometry, or of a selection.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DofReport {
    /// Remaining degrees of freedom: the constraint components touching the analysed objects,
    /// plus unconstrained objects' own parameters (`None` when too large to analyse).
    pub dof: Option<usize>,
    pub status: DofStatus,
    /// Equations that duplicate others.
    pub redundant: usize,
    /// Constraints taking part.
    pub constraints: usize,
    pub objects: Vec<ObjectDof>,
    pub message: String,
}

struct CompDof {
    dof: Option<usize>,
    redundant: usize,
    /// Per slot (model order).
    objects: Vec<Option<usize>>,
}

/// Compute component and object freedom only when rank and residual-evaluation work fit their budgets.
/// Return unknown freedom before building the Jacobian when either work limit is exceeded.
fn analyse(comp: &Comp) -> CompDof {
    let free = comp.free(None);
    let rows = comp.rows(None);
    let nf = free.len();
    let unknown = CompDof { dof: None, redundant: 0, objects: vec![None; comp.model.slots.len()] };
    if rows > lm::MAX_ROWS || (rows as f64) * (nf as f64) * (rows.min(nf) as f64) > MAX_RANK_WORK {
        return unknown;
    }
    // Only spline end frames are differentiated numerically, and each of those rebuilds its
    // spline. Analytic rows do not pay that cost, so vertex constraints on a large fit spline
    // stay analysable. A single expensive end frame still refuses before any reconstruction.
    let fallback = comp.built.iter().filter(|b| crate::jac::is_numeric_fallback(b));
    let mut fb_rows = 0usize;
    let mut fb_cost = 0.0f64;
    for b in fallback {
        fb_rows += b.rows;
        fb_cost += b.spline_reconstruct_cost(&comp.model);
    }
    let eval_work = (fb_rows as f64) * (nf as f64) * (1.0 + fb_cost);
    if fb_rows > 0 && eval_work > MAX_EVAL_WORK {
        return unknown;
    }
    let built: Vec<&crate::residual::Built> = comp.built.iter().collect();
    let jac = crate::jac::dense(&comp.model, &built, &comp.model.x0, &free);
    if nf > MAX_NULL_COLS {
        let rank = lm::rank_qr(&jac, rows, nf);
        return CompDof { dof: Some(nf - rank.min(nf)), redundant: rows.saturating_sub(rank), objects: vec![None; comp.model.slots.len()] };
    }
    let (rank, basis) = lm::null_space(&jac, rows, nf);
    let objects = comp
        .model
        .slots
        .iter()
        .map(|s| {
            let cols: Vec<usize> = free.iter().enumerate().filter(|(_, j)| (s.off..s.off + s.len).contains(*j)).map(|(k, _)| k).collect();
            if cols.is_empty() || basis.is_empty() {
                return Some(0);
            }
            // Null vectors restricted to the object's columns; their rank is its freedom.
            let restricted: Vec<Vec<f64>> = basis.iter().map(|v| cols.iter().map(|&k| v.get(k).copied().unwrap_or(0.0)).collect()).collect();
            Some(lm::qr(restricted, 0.0, 1e-6).0)
        })
        .collect();
    CompDof { dof: Some(nf - rank.min(nf)), redundant: rows.saturating_sub(rank), objects }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// Degrees of freedom of the constrained geometry (`handles` empty), or of the given objects
/// (constrained or not) and the constraint components they belong to.
pub fn dof(d: &Drawing, handles: &[Handle]) -> Result<DofReport, SolveError> {
    let cons: Vec<&Constraint> = d.constraints.iter().take(MAX_CONSTRAINTS).collect();
    let mut members_left = crate::model::MAX_RIGID_WORK;
    let (comps, _) = build_budgeted(d, &cons, &SolveOptions::default(), &mut members_left)?;
    let want = |h: Handle| handles.is_empty() || handles.contains(&h);
    let mut objects = Vec::new();
    let mut seen = HashSet::new();
    let mut total = Some(0usize);
    let mut redundant = 0;
    let mut constraints = 0;
    let mut any_constrained = false;
    for comp in &comps {
        if !comp.model.slots.iter().any(|s| want(s.handle)) {
            continue;
        }
        any_constrained = true;
        let a = analyse(comp);
        constraints += comp.built.len();
        redundant += a.redundant;
        total = match (total, a.dof) {
            (Some(t), Some(v)) => Some(t.saturating_add(v)),
            _ => None,
        };
        for (s, od) in comp.model.slots.iter().zip(&a.objects) {
            if !want(s.handle) || !seen.insert(s.handle) || objects.len() >= MAX_OBJECTS {
                continue;
            }
            let status = match od {
                _ if a.redundant > 0 => DofStatus::Over,
                Some(0) => DofStatus::Fully,
                Some(_) => DofStatus::Under,
                None => DofStatus::Unknown,
            };
            let kind = d.entity(s.handle).map_or("", |e| e.kind.type_name());
            objects.push(ObjectDof { handle: s.handle.hex(), kind, params: s.len, dof: *od, status });
        }
    }
    let mut free_objects = false;
    for &h in handles.iter().take(MAX_OBJECTS) {
        if seen.contains(&h) {
            continue;
        }
        let m = Model::build_budgeted(d, &[h], &mut members_left);
        let Some(s) = m.slots.first() else { continue };
        seen.insert(h);
        free_objects = true;
        total = total.map(|t| t.saturating_add(s.len));
        let kind = d.entity(h).map_or("", |e| e.kind.type_name());
        objects.push(ObjectDof { handle: h.hex(), kind, params: s.len, dof: Some(s.len), status: DofStatus::Unconstrained });
    }
    let status = if redundant > 0 {
        DofStatus::Over
    } else if !any_constrained {
        DofStatus::Unconstrained
    } else {
        match total {
            Some(0) => DofStatus::Fully,
            Some(_) => DofStatus::Under,
            None => DofStatus::Unknown,
        }
    };
    let message = match (status, total) {
        (DofStatus::Unconstrained, _) if !free_objects => "No constraints".to_string(),
        (DofStatus::Over, _) => format!("Over-constrained: {}", plural(redundant, "redundant equation", "redundant equations")),
        (DofStatus::Fully, _) => "Fully constrained".to_string(),
        (_, Some(n)) => format!("{} remaining", plural(n, "degree of freedom", "degrees of freedom")),
        (_, None) => "Too large to analyse".to_string(),
    };
    Ok(DofReport { dof: total, status, redundant, constraints, objects, message })
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    use crate::residual::{Built, Form};
    use cadcraft_doc::{Common, ConstraintKind, EntityKind, GeomRef, Space};
    use cadcraft_geom::{Spline, Vec2};

    /// An expensive fit-spline endpoint residual produces unknown freedom before numerical differentiation.
    #[test]
    fn endpoint_analysis_rejects_cubic_fit_work_before_differentiation() {
        let mut d = Drawing::default();
        let pts: Vec<_> = (0..500).map(|i| Vec2::new(i as f64, (i % 3) as f64)).collect();
        let mut spline = Spline::from_control(pts.clone(), 3);
        spline.fit = pts;
        let h = d.add(&Space::Model, Common::default(), EntityKind::Spline(spline)).unwrap();
        let model = Model::build(&d, &[h]);
        let endpoint = Built {
            id: 1,
            kind: ConstraintKind::Tangent,
            refs: vec![GeomRef::whole(h)],
            form: Form::EndOn { spline: 0, at_end: false },
            target: 0.0,
            sign: 1.0,
            rows: 2,
            aux_init: Vec::new(),
            aux: 0,
        };
        let comp = Comp { model, built: vec![endpoint], weights: vec![1.0; 2], held: Vec::new(), tol: 1e-9 };
        assert!(analyse(&comp).dof.is_none());
    }

    /// A point exactly on a circle centre. Central differences of `hypot` are 0, so the
    /// coincident row and the radius row both look like they only touch the radius and the
    /// finite-difference rank reports one redundant equation. The analytic subgradient does not.
    #[test]
    fn point_at_circle_centre_is_not_reported_redundant() {
        use cadcraft_doc::{Constraint, GeomRef, Point as DPoint};
        use cadcraft_geom::Vec3;
        let mut d = Drawing::default();
        let c = d
            .add(&Space::Model, Common::default(), EntityKind::Circle(cadcraft_doc::Circle { center: Vec3::new(0.0, 0.0, 0.0), radius: 5.0 }))
            .unwrap();
        let p = d.add(&Space::Model, Common::default(), EntityKind::Point(DPoint { p: Vec3::new(0.0, 0.0, 0.0), angle: 0.0 })).unwrap();
        d.constraints.push(Constraint { id: 1, kind: ConstraintKind::Radius, refs: vec![GeomRef::whole(c)], name: String::new(), expr: "5".into() });
        d.constraints.push(Constraint {
            id: 2,
            kind: ConstraintKind::Coincident,
            refs: vec![GeomRef::whole(p), GeomRef::whole(c)],
            name: String::new(),
            expr: String::new(),
        });
        let report = dof(&d, &[]).unwrap();
        assert_eq!(report.redundant, 0, "{report:?}");
        assert_ne!(report.status, DofStatus::Over);

        let cons: Vec<&Constraint> = d.constraints.iter().collect();
        let (comps, skip) = crate::build(&d, &cons, &SolveOptions::default()).unwrap();
        assert!(skip.is_empty(), "{skip:?}");
        let comp = &comps[0];
        let built: Vec<&crate::residual::Built> = comp.built.iter().collect();
        let free = comp.free(None);
        let rows = comp.rows(None);
        let analytic = crate::jac::dense(&comp.model, &built, &comp.model.x0, &free);
        let f = |x: &[f64], out: &mut Vec<f64>| comp.eval(x, out, None);
        let finite = lm::jacobian(&f, &comp.model.x0, &free, rows);
        let an_rank = lm::rank_qr(&analytic, rows, free.len());
        let fd_rank = lm::rank_qr(&finite, rows, free.len());
        assert_eq!(an_rank, rows, "analytic rank {an_rank}, jac {analytic:?}");
        assert_eq!(rows - fd_rank, 1, "finite-difference rank {fd_rank} should drop the radial column");
        assert!(an_rank > fd_rank);
    }
}
