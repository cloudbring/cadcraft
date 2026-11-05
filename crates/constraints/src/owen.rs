//! Graph-constructive split of one constraint component, before the global Gauss–Newton step.
//!
//! What this is: the entity graph (a constraint on two or more entities is a clique) is split
//! into biconnected components. A component is treated as rigid when its residual rows leave at
//! most three degrees of freedom on the entities' free parameters (`rows + 3 >= free`). A
//! full-column-rank system with two or more such clusters solves each cluster, then
//! the whole component. That is the simplified decomposition the solver uses.
//!
//! What this is not (Owen, "Algebraic solution of geometric constraint problems", 1991, is not
//! followed past that split):
//! - no ruler-and-compass construction sequence
//! - clusters are not merged into larger rigid bodies, and they are not reduced to 3-dof bodies
//!   that would shrink the global system — the global step still sees every residual
//! - no special cases for angles, tangency, or symmetry
//! - an underdetermined component is not warm-started, so the weighted minimum-norm step from
//!   the original parameters is unchanged
//! - graphs above 8192 entities, or a search deeper than 2048, skip the split

use std::collections::HashSet;

use crate::Comp;
use crate::lm::{self, Diff};
use crate::residual::Built;

const MAX_ENTITIES: usize = 8192;
const MAX_DEPTH: usize = 2048;
const MAX_RANK_WORK: usize = 4_000_000;

pub(crate) struct Decomp {
    /// Constraint indices of each rigid biconnected component, in discovery order.
    pub rigid: Vec<Vec<usize>>,
    /// `rigid` was solved before the global step.
    pub warm: bool,
}

pub(crate) fn decompose(comp: &Comp) -> Decomp {
    let free = comp.free(None);
    let rows = comp.rows(None);
    let n = comp.model.slots.len();
    let determined = rows >= free.len() && !free.is_empty();
    let Some(parts) = biconnected(&entity_graph(comp)) else {
        return Decomp { rigid: Vec::new(), warm: false };
    };
    let mark = free_mark(&free, comp.model.x0.len());
    let mut rigid = Vec::new();
    for nodes in parts {
        if let Some(idxs) = rigid_constraints(comp, &nodes, &mark) {
            rigid.push(idxs);
        }
    }
    let rank_work = rows.saturating_mul(free.len()).saturating_mul(rows.min(free.len()));
    let warm = determined
        && rigid.len() >= 2
        && n <= MAX_ENTITIES
        && rank_work <= MAX_RANK_WORK
        && comp.built.iter().all(|built| !crate::jac::is_numeric_fallback(built))
        && {
            let built: Vec<&Built> = comp.built.iter().collect();
            let jacobian = crate::jac::dense(&comp.model, &built, &comp.model.x0, &free);
            lm::rank_qr(&jacobian, rows, free.len(), lm::COL_NOISE) == free.len()
        };
    Decomp { rigid, warm }
}

/// Parameters to give the global step. A warm start only when [`decompose`] says so.
pub(crate) fn starting_x(comp: &Comp) -> Vec<f64> {
    let free_count = comp.free(None).len();
    let rows = comp.rows(None);
    if free_count == 0 || rows < free_count || rows.saturating_mul(free_count).saturating_mul(rows.min(free_count)) > MAX_RANK_WORK {
        return comp.model.x0.clone();
    }
    let decomp = decompose(comp);
    if !decomp.warm {
        return comp.model.x0.clone();
    }
    let mut x = comp.model.x0.clone();
    let free = comp.free(None);
    let mark = free_mark(&free, x.len());
    for idxs in &decomp.rigid {
        if let Some(xn) = solve_cluster(comp, idxs, &x, &mark) {
            x = xn;
        }
    }
    x
}

fn entity_graph(comp: &Comp) -> Vec<Vec<usize>> {
    let n = comp.model.slots.len();
    let mut adj = vec![Vec::new(); n];
    if n > MAX_ENTITIES {
        return adj;
    }
    for b in &comp.built {
        let mut nodes = Vec::new();
        for r in &b.refs {
            if let Some(i) = comp.model.slots.iter().position(|s| s.handle == r.handle) {
                nodes.push(i);
            }
        }
        nodes.sort_unstable();
        nodes.dedup();
        for (a, &u) in nodes.iter().enumerate() {
            for &v in nodes.iter().skip(a + 1) {
                if let Some(row) = adj.get_mut(u) {
                    row.push(v);
                }
                if let Some(row) = adj.get_mut(v) {
                    row.push(u);
                }
            }
        }
    }
    for row in &mut adj {
        row.sort_unstable();
        row.dedup();
    }
    adj
}

fn free_mark(free: &[usize], n: usize) -> Vec<bool> {
    let mut mark = vec![false; n];
    for &j in free {
        if let Some(slot) = mark.get_mut(j) {
            *slot = true;
        }
    }
    mark
}

fn rigid_constraints(comp: &Comp, nodes: &[usize], mark: &[bool]) -> Option<Vec<usize>> {
    if nodes.len() < 2 {
        return None;
    }
    let set: HashSet<usize> = nodes.iter().copied().collect();
    let mut idxs = Vec::new();
    let mut rows = 0usize;
    for (i, b) in comp.built.iter().enumerate() {
        let touched = constraint_nodes(comp, b);
        if touched.is_empty() || touched.iter().any(|n| !set.contains(n)) {
            continue;
        }
        idxs.push(i);
        rows += b.rows;
    }
    let params = nodes
        .iter()
        .filter_map(|&n| comp.model.slots.get(n))
        .map(|s| (s.off..s.off + s.len).filter(|&j| mark.get(j).copied() == Some(true)).count())
        .sum::<usize>();
    (rows > 0 && rows + 3 >= params).then_some(idxs)
}

fn constraint_nodes(comp: &Comp, b: &Built) -> Vec<usize> {
    let mut nodes = Vec::new();
    for r in &b.refs {
        if let Some(i) = comp.model.slots.iter().position(|s| s.handle == r.handle) {
            nodes.push(i);
        }
    }
    nodes.sort_unstable();
    nodes.dedup();
    nodes
}

fn solve_cluster(comp: &Comp, idxs: &[usize], x: &[f64], mark: &[bool]) -> Option<Vec<f64>> {
    let built: Vec<&Built> = idxs.iter().filter_map(|&i| comp.built.get(i)).collect();
    let mut free = Vec::new();
    for b in &built {
        free.extend((b.aux..b.aux + b.aux_init.len()).filter(|&index| mark.get(index).copied() == Some(true)));
        for n in constraint_nodes(comp, b) {
            let Some(s) = comp.model.slots.get(n) else { continue };
            for j in s.off..s.off + s.len {
                if mark.get(j).copied() == Some(true) {
                    free.push(j);
                }
            }
        }
    }
    free.sort_unstable();
    free.dedup();
    if free.is_empty() || built.is_empty() {
        return None;
    }
    let f = |xx: &[f64], out: &mut Vec<f64>| {
        for b in &built {
            b.eval(&comp.model, xx, out);
        }
    };
    let diff = Diff::Analytic { model: &comp.model, built: &built };
    let o = lm::solve_diff(&f, x, &free, &comp.weights, comp.tol, crate::MAX_ITER, &diff);
    (o.converged && !o.too_wide).then_some(o.x)
}

fn biconnected(adj: &[Vec<usize>]) -> Option<Vec<Vec<usize>>> {
    if adj.len() > MAX_ENTITIES {
        return None;
    }
    let mut search = Tarjan { adj, disc: vec![None; adj.len()], low: vec![0; adj.len()], stack: Vec::new(), comps: Vec::new(), time: 0, deep: false };
    for u in 0..adj.len() {
        if search.disc.get(u).and_then(|d| *d).is_none() {
            search.visit(u, None, 0);
            search.flush();
        }
    }
    (!search.deep).then_some(search.comps)
}

struct Tarjan<'a> {
    adj: &'a [Vec<usize>],
    disc: Vec<Option<usize>>,
    low: Vec<usize>,
    stack: Vec<(usize, usize)>,
    comps: Vec<Vec<usize>>,
    time: usize,
    deep: bool,
}

impl Tarjan<'_> {
    fn visit(&mut self, u: usize, parent: Option<usize>, depth: usize) {
        if self.deep {
            return;
        }
        if depth > MAX_DEPTH {
            self.deep = true;
            return;
        }
        let at = self.time;
        self.time += 1;
        if let Some(slot) = self.disc.get_mut(u) {
            *slot = Some(at);
        }
        if let Some(slot) = self.low.get_mut(u) {
            *slot = at;
        }
        let mut children = 0usize;
        let nbrs = self.adj.get(u).cloned().unwrap_or_default();
        for v in nbrs {
            if self.disc.get(v).and_then(|d| *d).is_none() {
                children += 1;
                self.stack.push((u, v));
                self.visit(v, Some(u), depth + 1);
                let low_v = self.low.get(v).copied().unwrap_or(0);
                if let Some(low_u) = self.low.get_mut(u) {
                    *low_u = (*low_u).min(low_v);
                }
                let root = parent.is_none() && children > 1;
                let cut = parent.is_some() && low_v >= at;
                if root || cut {
                    self.pop_until(u, v);
                }
            } else if Some(v) != parent && self.disc.get(v).and_then(|d| *d).unwrap_or(usize::MAX) < at {
                if let Some(low_u) = self.low.get_mut(u) {
                    *low_u = (*low_u).min(self.disc.get(v).and_then(|d| *d).unwrap_or(at));
                }
                self.stack.push((u, v));
            }
        }
    }

    fn flush(&mut self) {
        if self.stack.is_empty() {
            return;
        }
        let mut nodes = Vec::new();
        while let Some((a, b)) = self.stack.pop() {
            nodes.push(a);
            nodes.push(b);
        }
        self.keep(nodes);
    }

    fn pop_until(&mut self, u: usize, v: usize) {
        let mut nodes = Vec::new();
        while let Some((a, b)) = self.stack.pop() {
            nodes.push(a);
            nodes.push(b);
            if a == u && b == v {
                break;
            }
        }
        self.keep(nodes);
    }

    fn keep(&mut self, mut nodes: Vec<usize>) {
        nodes.sort_unstable();
        nodes.dedup();
        if nodes.len() >= 2 {
            self.comps.push(nodes);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SolveOptions, build};
    use cadcraft_doc::{Common, Constraint, ConstraintKind, DistAxis, Drawing, EntityKind, GeomRef, Point as DPoint, Space};
    use cadcraft_geom::Vec3;

    fn point(d: &mut Drawing, x: f64, y: f64) -> cadcraft_doc::Handle {
        d.add(&Space::Model, Common::default(), EntityKind::Point(DPoint { p: Vec3::new(x, y, 0.0), angle: 0.0 })).unwrap()
    }

    fn distance(d: &mut Drawing, id: u32, a: cadcraft_doc::Handle, b: cadcraft_doc::Handle, len: f64) {
        d.constraints.push(Constraint {
            id,
            kind: ConstraintKind::Distance(DistAxis::Aligned),
            refs: vec![GeomRef::whole(a), GeomRef::whole(b)],
            name: String::new(),
            expr: format!("{len:.8}"),
        });
    }

    fn comp_of(d: &Drawing) -> crate::Comp {
        let cons: Vec<&Constraint> = d.constraints.iter().collect();
        let (mut comps, skip) = build(d, &cons, &SolveOptions::default()).unwrap();
        assert!(skip.is_empty(), "{skip:?}");
        assert_eq!(comps.len(), 1);
        comps.pop().unwrap()
    }

    #[test]
    fn a_chain_is_two_bars_and_is_not_warm_started() {
        let mut d = Drawing::default();
        let a = point(&mut d, 0.0, 0.0);
        let b = point(&mut d, 3.0, 0.2);
        let c = point(&mut d, 6.1, 0.4);
        distance(&mut d, 1, a, b, 3.0);
        distance(&mut d, 2, b, c, 3.0);
        let comp = comp_of(&d);
        let decomp = decompose(&comp);
        assert_eq!(decomp.rigid.len(), 2, "{:?}", decomp.rigid);
        assert!(!decomp.warm);
    }

    #[test]
    fn one_triangle_is_a_single_cluster() {
        let mut d = Drawing::default();
        let a = point(&mut d, 0.0, 0.0);
        let b = point(&mut d, 4.0, 0.1);
        let c = point(&mut d, 1.0, 3.0);
        distance(&mut d, 1, a, b, 4.0);
        distance(&mut d, 2, b, c, 4.2);
        distance(&mut d, 3, c, a, 3.2);
        let comp = comp_of(&d);
        let decomp = decompose(&comp);
        assert_eq!(decomp.rigid.len(), 1);
        assert!(!decomp.warm);
    }

    #[test]
    fn two_triangles_that_share_a_vertex_stay_on_the_minimum_norm_path() {
        let mut d = Drawing::default();
        let a = point(&mut d, 0.0, 0.0);
        let b = point(&mut d, 4.0, 0.2);
        let c = point(&mut d, 1.2, 3.4);
        let e = point(&mut d, -1.0, 3.6);
        let f = point(&mut d, -3.5, 0.4);
        distance(&mut d, 1, a, b, 4.0);
        distance(&mut d, 2, b, c, 4.5);
        distance(&mut d, 3, c, a, 3.6);
        distance(&mut d, 4, a, e, 3.8);
        distance(&mut d, 5, e, f, 4.1);
        distance(&mut d, 6, f, a, 3.5);
        let comp = comp_of(&d);
        let decomp = decompose(&comp);
        assert_eq!(decomp.rigid.len(), 2, "{:?}", decomp.rigid);
        assert!(!decomp.warm, "an underdetermined hinge keeps the global minimum-norm step");
    }

    #[test]
    fn a_determined_pair_of_clusters_is_warm_started_and_converges() {
        let mut d = Drawing::default();
        let len = |p: (f64, f64), q: (f64, f64)| (p.0 - q.0).hypot(p.1 - q.1);
        let a = point(&mut d, 0.0, 0.0);
        let b = point(&mut d, 5.0, 0.0);
        // Stored off the targets. Two fixes, a vertical, and the six distances make a square system.
        let c = point(&mut d, 1.3, 3.7);
        let e = point(&mut d, 0.2, 5.3);
        let f = point(&mut d, -3.7, 1.2);
        distance(&mut d, 1, a, b, len((0.0, 0.0), (5.0, 0.0)));
        distance(&mut d, 2, b, c, len((5.0, 0.0), (1.0, 4.0)));
        distance(&mut d, 3, c, a, len((1.0, 4.0), (0.0, 0.0)));
        distance(&mut d, 4, a, e, len((0.0, 0.0), (0.0, 5.0)));
        distance(&mut d, 5, e, f, len((0.0, 5.0), (-4.0, 1.0)));
        distance(&mut d, 6, f, a, len((-4.0, 1.0), (0.0, 0.0)));
        for (id, h, kind, refs) in [
            (7, a, ConstraintKind::Fix, vec![GeomRef::whole(a)]),
            (8, b, ConstraintKind::Fix, vec![GeomRef::whole(b)]),
            (9, e, ConstraintKind::Vertical, vec![GeomRef::whole(a), GeomRef::whole(e)]),
        ] {
            let _ = h;
            d.constraints.push(Constraint { id, kind, refs, name: String::new(), expr: String::new() });
        }
        let comp = comp_of(&d);
        let decomp = decompose(&comp);
        assert!(decomp.warm, "rigid {:?}, rows {} free {}", decomp.rigid.len(), comp.rows(None), comp.free(None).len());
        let o = comp.run(None).unwrap();
        assert!(o.converged);
        let free = comp.free(None);
        let built: Vec<&Built> = comp.built.iter().collect();
        let residual = |parameters: &[f64], output: &mut Vec<f64>| comp.eval(parameters, output, None);
        let global = lm::solve_diff(
            &residual,
            &comp.model.x0,
            &free,
            &comp.weights,
            comp.tol,
            crate::MAX_ITER,
            &Diff::Analytic { model: &comp.model, built: &built },
        );
        assert!(global.converged);
        let delta = o.x.iter().zip(&global.x).map(|(clustered, unclustered)| (clustered - unclustered).abs()).fold(0.0, f64::max);
        assert!(delta <= 1e-9, "cluster delta {delta:e}");
        for omitted in 0..comp.built.len() {
            let free = comp.free(Some(omitted));
            let built: Vec<&Built> = comp.built.iter().enumerate().filter(|(index, _)| *index != omitted).map(|(_, built)| built).collect();
            let residual = |parameters: &[f64], output: &mut Vec<f64>| comp.eval(parameters, output, Some(omitted));
            let direct = lm::solve_diff(
                &residual,
                &comp.model.x0,
                &free,
                &comp.weights,
                comp.tol,
                crate::MAX_ITER,
                &Diff::Analytic { model: &comp.model, built: &built },
            );
            let probe = comp.run(Some(omitted)).unwrap();
            assert_eq!(probe.converged, direct.converged);
            assert_eq!(probe.x, direct.x, "conflict probe must not solve the omitted constraint first");
        }
        comp.model.write_back(&mut d, &o.x);
        let at = |h| match &d.entity(h).unwrap().kind {
            EntityKind::Point(p) => (p.p.x, p.p.y),
            _ => panic!(),
        };
        let near = |h, x: f64, y: f64| {
            let p = at(h);
            assert!((p.0 - x).abs() < 1e-6 && (p.1 - y).abs() < 1e-6, "{p:?} vs ({x}, {y})");
        };
        near(a, 0.0, 0.0);
        near(b, 5.0, 0.0);
        near(c, 1.0, 4.0);
        near(e, 0.0, 5.0);
        near(f, -4.0, 1.0);
    }

    #[test]
    fn redundant_rows_do_not_make_a_hinge_determined() {
        let mut drawing = Drawing::default();
        let first = point(&mut drawing, 0.0, 0.0);
        let hinge = point(&mut drawing, 3.0, 0.2);
        let last = point(&mut drawing, 6.1, 0.4);
        for index in 0..3 {
            distance(&mut drawing, 2 * index + 1, first, hinge, 3.0);
            distance(&mut drawing, 2 * index + 2, hinge, last, 3.0);
        }
        let component = comp_of(&drawing);
        assert_eq!(component.rows(None), component.free(None).len());
        assert!(!decompose(&component).warm);
        assert_eq!(starting_x(&component), component.model.x0);
    }
}
