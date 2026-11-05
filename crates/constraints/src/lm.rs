//! Damped, weighted minimum-norm Gauss–Newton (Levenberg–Marquardt style).
//!
//! The Jacobian is analytic ([`crate::jac`]) when the caller passes the constraint model, and
//! central differences otherwise (spline end frames, and callers that only have a residual
//! closure). Each step solves `min ‖W^{1/2} Δx‖` subject to `J Δx = −r` (damped):
//! `Δx = −W⁻¹ Jᵀ (J W⁻¹ Jᵀ + μ I)⁻¹ r`. Under-constrained systems therefore move as little as
//! possible, and heavily weighted parameters (geometry the user just edited, or the first object
//! picked) move least.
//!
//! Up to [`DENSE_LIMIT`] rows the normal matrix is eliminated densely. Above that it is an
//! envelope Cholesky ([`crate::sparse`]); `faer` is not a dependency.

/// Most residual rows one solve may have. The dense cap of 800 is gone: past [`DENSE_LIMIT`]
/// rows the factor is banded, and a pattern wider than [`crate::sparse::MAX_BAND`] is
/// [`Outcome::too_wide`] instead of an allocation of `rows²`.
pub const MAX_ROWS: usize = 20_000;

/// Normal matrices at or below this size stay on Gaussian elimination.
pub(crate) const DENSE_LIMIT: usize = 48;

pub(crate) struct Outcome {
    pub x: Vec<f64>,
    pub converged: bool,
    /// The normal-matrix envelope did not fit. The caller reports [`crate::SolveError::TooLarge`].
    pub too_wide: bool,
}

fn outcome(x: Vec<f64>, converged: bool) -> Outcome {
    Outcome { x, converged, too_wide: false }
}

pub(crate) fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0f64, |m, a| if a.is_finite() { m.max(a.abs()) } else { f64::INFINITY })
}

fn norm2(v: &[f64]) -> f64 {
    v.iter().map(|a| a * a).sum::<f64>()
}

/// Numerical Jacobian over the free parameters (row-major, rows × free.len()).
pub(crate) fn jacobian(f: &dyn Fn(&[f64], &mut Vec<f64>), x: &[f64], free: &[usize], rows: usize) -> Vec<f64> {
    let nf = free.len();
    let mut jac = vec![0.0; rows * nf];
    let mut xp = x.to_vec();
    let (mut rp, mut rm) = (Vec::with_capacity(rows), Vec::with_capacity(rows));
    for (k, &j) in free.iter().enumerate() {
        let Some(&xj) = x.get(j) else { continue };
        let h = 1e-7 * xj.abs().max(1.0);
        if let Some(v) = xp.get_mut(j) {
            *v = xj + h;
        }
        rp.clear();
        f(&xp, &mut rp);
        if let Some(v) = xp.get_mut(j) {
            *v = xj - h;
        }
        rm.clear();
        f(&xp, &mut rm);
        if let Some(v) = xp.get_mut(j) {
            *v = xj;
        }
        for i in 0..rows {
            let d = (rp.get(i).copied().unwrap_or(0.0) - rm.get(i).copied().unwrap_or(0.0)) / (2.0 * h);
            if let Some(slot) = jac.get_mut(i * nf + k) {
                *slot = if d.is_finite() { d } else { 0.0 };
            }
        }
    }
    jac
}

/// Solve the dense system `a y = b` (n × n, row-major) by Gaussian elimination with partial
/// pivoting. `None` when singular.
fn solve_dense(mut a: Vec<f64>, mut b: Vec<f64>, n: usize) -> Option<Vec<f64>> {
    let scale = a.iter().fold(0.0f64, |m, v| m.max(v.abs())).max(1e-300);
    for col in 0..n {
        let mut piv = col;
        let mut best = 0.0;
        for r in col..n {
            let v = a.get(r * n + col)?.abs();
            if v > best {
                best = v;
                piv = r;
            }
        }
        if best <= scale * 1e-15 {
            return None;
        }
        if piv != col {
            for c in 0..n {
                a.swap(piv * n + c, col * n + c);
            }
            b.swap(piv, col);
        }
        let p = *a.get(col * n + col)?;
        for r in col + 1..n {
            let f = *a.get(r * n + col)? / p;
            if f == 0.0 {
                continue;
            }
            for c in col..n {
                let v = *a.get(col * n + c)?;
                *a.get_mut(r * n + c)? -= f * v;
            }
            let bv = *b.get(col)?;
            *b.get_mut(r)? -= f * bv;
        }
    }
    let mut y = vec![0.0; n];
    for r in (0..n).rev() {
        let mut s = *b.get(r)?;
        for c in r + 1..n {
            s -= a.get(r * n + c)? * y.get(c)?;
        }
        *y.get_mut(r)? = s / a.get(r * n + r)?;
    }
    y.iter().all(|v| v.is_finite()).then_some(y)
}

/// Householder QR with column pivoting of the columns `cols` (each of the same length). Stops
/// when the largest remaining column norm drops to `rel` times the first pivot's (or below
/// `abs`). Returns the numerical rank and the Householder vectors (full length, zero above the
/// diagonal; each reflector is `I − 2vvᵀ/(vᵀv)`).
pub(crate) fn qr(mut cols: Vec<Vec<f64>>, rel: f64, abs: f64) -> (usize, Vec<Vec<f64>>) {
    let m = cols.first().map_or(0, Vec::len);
    let n = cols.len();
    let mut refl = Vec::new();
    let mut first = None;
    for k in 0..m.min(n) {
        // Pivot: the column with the largest remaining norm.
        let mut best = (k, -1.0f64);
        for (j, c) in cols.iter().enumerate().skip(k) {
            let v = c.get(k..).map_or(0.0, norm2);
            if v.is_finite() && v > best.1 {
                best = (j, v);
            }
        }
        let norm = best.1.max(0.0).sqrt();
        let lim = first.map_or(abs, |f: f64| (f * rel).max(abs));
        if crate::curves::not_above(norm, lim) {
            break;
        }
        first.get_or_insert(norm);
        cols.swap(k, best.0);
        let Some(col) = cols.get(k) else { break };
        let mut v = vec![0.0; m];
        for i in k..m {
            if let (Some(d), Some(s)) = (v.get_mut(i), col.get(i)) {
                *d = *s;
            }
        }
        let x0 = v.get(k).copied().unwrap_or(0.0);
        let alpha = if x0 >= 0.0 { -norm } else { norm };
        if let Some(d) = v.get_mut(k) {
            *d -= alpha;
        }
        let vv = norm2(&v);
        if crate::curves::not_above(vv, 0.0) {
            break;
        }
        for c in cols.iter_mut().skip(k) {
            reflect(&v, vv, c);
        }
        refl.push(v);
    }
    (refl.len(), refl)
}

fn reflect(v: &[f64], vv: f64, c: &mut [f64]) {
    let d: f64 = v.iter().zip(c.iter()).map(|(a, b)| a * b).sum();
    let f = 2.0 * d / vv;
    if f != 0.0 {
        for (ci, vi) in c.iter_mut().zip(v) {
            *ci -= f * vi;
        }
    }
}

/// Rows and columns scaled to unit norm (zero ones left alone), as columns of the transpose:
/// `out[i]` is row `i` of the row-major `rows × cols` matrix `m`.
fn balanced_rows(m: &[f64], rows: usize, cols: usize) -> Vec<Vec<f64>> {
    let mut r: Vec<Vec<f64>> = (0..rows).map(|i| m.get(i * cols..(i + 1) * cols).map(<[f64]>::to_vec).unwrap_or_else(|| vec![0.0; cols])).collect();
    for row in &mut r {
        let n = norm2(row).sqrt();
        let f = if n > 1e-12 && n.is_finite() { 1.0 / n } else { 0.0 };
        row.iter_mut().for_each(|v| *v *= f);
    }
    // Columns: unit norm, except numerical noise (an exactly zero derivative evaluated by
    // differences), which scaling would turn into a spurious independent direction.
    for c in 0..cols {
        let n = r.iter().map(|row| row.get(c).map_or(0.0, |v| v * v)).sum::<f64>().sqrt();
        let f = if n > COL_NOISE && n.is_finite() { 1.0 / n } else { 0.0 };
        for row in &mut r {
            if let Some(v) = row.get_mut(c) {
                *v *= f;
            }
        }
    }
    r
}

/// Column norm (after row balancing) below which a column is difference noise.
const COL_NOISE: f64 = 1e-6;
/// Relative threshold below which a pivot counts as zero (the Jacobian is numerical).
const RANK_TOL: f64 = 1e-7;

/// Numerical rank of a row-major `rows × cols` Jacobian (rows and columns balanced first,
/// Householder QR with column pivoting).
pub(crate) fn rank_qr(m: &[f64], rows: usize, cols: usize) -> usize {
    if rows == 0 || cols == 0 {
        return 0;
    }
    // QR of the transpose: its columns are the Jacobian's rows.
    qr(balanced_rows(m, rows, cols), RANK_TOL, 1e-12).0
}

/// Rank and an orthonormal basis of the null space (each vector of length `cols`) of a
/// row-major `rows × cols` Jacobian, in balanced column coordinates (a diagonal scaling, so
/// which parameters a null vector moves is preserved).
pub(crate) fn null_space(m: &[f64], rows: usize, cols: usize) -> (usize, Vec<Vec<f64>>) {
    let (rank, refl) = if rows == 0 { (0, Vec::new()) } else { qr(balanced_rows(m, rows, cols), RANK_TOL, 1e-12) };
    let mut basis = Vec::with_capacity(cols.saturating_sub(rank));
    for j in rank..cols {
        let mut e = vec![0.0; cols];
        if let Some(v) = e.get_mut(j) {
            *v = 1.0;
        }
        // Q e_j with Q = H_0 H_1 … H_{r−1}.
        for v in refl.iter().rev() {
            let vv = norm2(v);
            if vv > 0.0 {
                reflect(v, vv, &mut e);
            }
        }
        basis.push(e);
    }
    (rank, basis)
}

/// Where a step's Jacobian comes from.
pub(crate) enum Diff<'a> {
    /// Central differences of `f` ([`jacobian`]).
    Numeric,
    /// Closed-form partials. `built` is the constraints `f` evaluates, in order.
    Analytic { model: &'a crate::model::Model, built: &'a [&'a crate::residual::Built] },
}

/// Run the solver from `x0` with a numerical Jacobian. Only `free` parameters change.
///
/// Production solves pass [`Diff::Analytic`] to [`solve_diff`]. This entry stays for the
/// minimum-norm unit tests, which have no constraint model.
#[allow(dead_code)]
pub(crate) fn solve(f: &dyn Fn(&[f64], &mut Vec<f64>), x0: &[f64], free: &[usize], weights: &[f64], tol: f64, max_iter: usize) -> Outcome {
    solve_diff(f, x0, free, weights, tol, max_iter, &Diff::Numeric)
}

struct LoopIn<'a> {
    f: &'a dyn Fn(&[f64], &mut Vec<f64>),
    x0: &'a [f64],
    free: &'a [usize],
    weights: &'a [f64],
    diff: &'a Diff<'a>,
    tol: f64,
    max_iter: usize,
    dense_limit: usize,
}

/// Run the solver from `x0`. Only `free` parameters change.
pub(crate) fn solve_diff(
    f: &dyn Fn(&[f64], &mut Vec<f64>),
    x0: &[f64],
    free: &[usize],
    weights: &[f64],
    tol: f64,
    max_iter: usize,
    diff: &Diff<'_>,
) -> Outcome {
    solve_loop(LoopIn { f, x0, free, weights, diff, tol, max_iter, dense_limit: DENSE_LIMIT })
}

fn solve_loop(input: LoopIn<'_>) -> Outcome {
    let LoopIn { f, x0, free, weights, diff, tol, max_iter, dense_limit } = input;
    let mut x = x0.to_vec();
    let mut r = Vec::new();
    f(&x, &mut r);
    let rows = r.len();
    if rows == 0 || max_abs(&r) <= tol {
        return outcome(x, true);
    }
    if free.is_empty() || rows > MAX_ROWS {
        return outcome(x, false);
    }
    let nf = free.len();
    let winv: Vec<f64> = free.iter().map(|&j| 1.0 / weights.get(j).copied().unwrap_or(1.0).max(1e-12)).collect();
    let mut mu = 1e-10;
    let mut cost = norm2(&r);
    let mut iter = 0;
    let mut rn = Vec::with_capacity(rows);
    while iter < max_iter {
        iter += 1;
        let step = if rows <= dense_limit {
            StepJac::Dense(match diff {
                Diff::Numeric => jacobian(f, &x, free, rows),
                Diff::Analytic { model, built } => crate::jac::dense(model, built, &x, free),
            })
        } else {
            StepJac::Trips(match diff {
                Diff::Numeric => numeric_trips(f, &x, free, rows),
                Diff::Analytic { model, built } => crate::jac::entries(model, built, &x, free).triplets().to_vec(),
            })
        };
        let mut improved = false;
        for _ in 0..12 {
            let y = match &step {
                StepJac::Dense(jac) => match dense_normal(jac, &winv, &r, rows, nf, mu) {
                    Some(y) => y,
                    None => {
                        mu *= 10.0;
                        continue;
                    }
                },
                StepJac::Trips(trips) => match crate::sparse::solve_damped(rows, trips, &winv, &r, mu) {
                    Ok(y) => y,
                    Err(crate::sparse::FactorError::Singular) => {
                        mu *= 10.0;
                        continue;
                    }
                    Err(crate::sparse::FactorError::TooWide) => return Outcome { x, converged: false, too_wide: true },
                },
            };
            let xn = descend(&x, free, &winv, &step, &y);
            rn.clear();
            f(&xn, &mut rn);
            let cn = norm2(&rn);
            if cn.is_finite() && cn < cost {
                x = xn;
                std::mem::swap(&mut r, &mut rn);
                cost = cn;
                mu = (mu * 0.1).max(1e-15);
                improved = true;
                break;
            }
            mu *= 10.0;
        }
        if max_abs(&r) <= tol {
            return outcome(x, true);
        }
        if !improved {
            break;
        }
    }
    outcome(x, max_abs(&r) <= tol)
}

enum StepJac {
    Dense(Vec<f64>),
    Trips(Vec<(u32, u32, f64)>),
}

fn numeric_trips(f: &dyn Fn(&[f64], &mut Vec<f64>), x: &[f64], free: &[usize], rows: usize) -> Vec<(u32, u32, f64)> {
    let jac = jacobian(f, x, free, rows);
    let nf = free.len();
    let mut trips = Vec::new();
    for i in 0..rows {
        for c in 0..nf {
            let v = jac.get(i * nf + c).copied().unwrap_or(0.0);
            if v != 0.0 && v.is_finite() {
                let (Ok(r), Ok(col)) = (u32::try_from(i), u32::try_from(c)) else { continue };
                trips.push((r, col, v));
            }
        }
    }
    trips
}

/// Dense `J W⁻¹ Jᵀ + μ D I`, Gaussian elimination. Same accumulation order as before the
/// sparse factor, so a system inside [`DENSE_LIMIT`] takes the same steps.
pub(crate) fn dense_normal(jac: &[f64], winv: &[f64], r: &[f64], rows: usize, nf: usize, mu: f64) -> Option<Vec<f64>> {
    let mut a = vec![0.0; rows.saturating_mul(rows)];
    for i in 0..rows {
        for k in i..rows {
            let mut s = 0.0;
            for (c, w) in winv.iter().enumerate() {
                s += jac.get(i * nf + c).copied().unwrap_or(0.0) * jac.get(k * nf + c).copied().unwrap_or(0.0) * w;
            }
            if let Some(v) = a.get_mut(i * rows + k) {
                *v = s;
            }
            if let Some(v) = a.get_mut(k * rows + i) {
                *v = s;
            }
        }
    }
    let diag_max = (0..rows).map(|i| a.get(i * rows + i).copied().unwrap_or(0.0)).fold(0.0_f64, f64::max).max(1e-12);
    for i in 0..rows {
        if let Some(v) = a.get_mut(i * rows + i) {
            *v += mu * diag_max;
        }
    }
    solve_dense(a, r.to_vec(), rows)
}

fn descend(x: &[f64], free: &[usize], winv: &[f64], step: &StepJac, y: &[f64]) -> Vec<f64> {
    let mut acc = vec![0.0; free.len()];
    match step {
        StepJac::Dense(jac) => {
            let nf = free.len();
            for (c, slot) in acc.iter_mut().enumerate() {
                for (i, yi) in y.iter().enumerate() {
                    *slot += jac.get(i * nf + c).copied().unwrap_or(0.0) * yi;
                }
            }
        }
        StepJac::Trips(trips) => {
            for &(r, c, v) in trips {
                if let Some(slot) = acc.get_mut(c as usize) {
                    *slot += v * y.get(r as usize).copied().unwrap_or(0.0);
                }
            }
        }
    }
    let mut xn = x.to_vec();
    for (c, &j) in free.iter().enumerate() {
        if let Some(v) = xn.get_mut(j) {
            *v -= winv.get(c).copied().unwrap_or(1.0) * acc.get(c).copied().unwrap_or(0.0);
        }
    }
    xn
}

/// Dense Gaussian elimination of a numerical Jacobian, with no row cap below [`MAX_ROWS`].
/// The ignored chain benchmark times this against the analytic envelope factor.
#[cfg(test)]
pub(crate) fn solve_dense_legacy(
    f: &dyn Fn(&[f64], &mut Vec<f64>),
    x0: &[f64],
    free: &[usize],
    weights: &[f64],
    tol: f64,
    max_iter: usize,
) -> Outcome {
    solve_loop(LoopIn { f, x0, free, weights, diff: &Diff::Numeric, tol, max_iter, dense_limit: usize::MAX })
}

#[cfg(test)]
pub(crate) fn solve_fixture(comp: &crate::Comp, sparse: bool) -> Outcome {
    let built: Vec<&crate::residual::Built> = comp.built.iter().collect();
    let free = comp.free(None);
    let residual = |parameters: &[f64], output: &mut Vec<f64>| comp.eval(parameters, output, None);
    solve_loop(LoopIn {
        f: &residual,
        x0: &comp.model.x0,
        free: &free,
        weights: &comp.weights,
        diff: &Diff::Analytic { model: &comp.model, built: &built },
        tol: comp.tol,
        max_iter: crate::MAX_ITER,
        dense_limit: if sparse { 0 } else { usize::MAX },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_normal_matches_dense_with_weights_permutations_and_fill() {
        let rows = 73;
        let columns = rows * 2;
        let weights: Vec<f64> = (0..columns).map(|index| 0.2 + (index % 11) as f64 * 0.3).collect();
        let rhs: Vec<f64> = (0..rows).map(|index| (index as f64 * 0.37).sin()).collect();
        let mut triplets = Vec::new();
        let mut jacobian = vec![0.0; rows * columns];
        for row in 0..rows {
            let permuted = row * 17 % rows;
            for (column, value) in [(row, 2.0), (rows + row, 0.3), (rows + (row + 1) % rows, -0.4), (rows + (row + 3) % rows, 0.2)] {
                triplets.push((permuted as u32, column as u32, value));
                jacobian[permuted * columns + column] = value;
            }
        }
        for damping in [1e-4, 1e-8, 1e-10, 1e-15] {
            let dense = dense_normal(&jacobian, &weights, &rhs, rows, columns, damping).unwrap();
            let sparse = crate::sparse::solve_damped(rows, &triplets, &weights, &rhs, damping).unwrap();
            let delta = dense.iter().zip(&sparse).map(|(dense, sparse)| (dense - sparse).abs()).fold(0.0, f64::max);
            assert!(delta <= 1e-9, "normal solve delta {delta:e} at damping {damping:e}");
            eprintln!("normal solve delta {delta:e} at damping {damping:e}");
        }
    }

    #[test]
    fn dense_and_rank() {
        let y = solve_dense(vec![2.0, 1.0, 1.0, 3.0], vec![3.0, 5.0], 2).unwrap();
        assert!((y[0] - 0.8).abs() < 1e-12 && (y[1] - 1.4).abs() < 1e-12);
        assert!(solve_dense(vec![1.0, 2.0, 2.0, 4.0], vec![1.0, 1.0], 2).is_none());
        assert_eq!(rank_qr(&[1.0, 0.0, 0.0, 1.0, 1.0, 1.0], 3, 2), 2);
        assert_eq!(rank_qr(&[1.0, 2.0, 2.0, 4.0], 2, 2), 1);
        assert_eq!(rank_qr(&[0.0; 4], 2, 2), 0);
        assert_eq!(rank_qr(&[1e-9, 0.0, 0.0, 1e6], 2, 2), 2, "balanced: scale does not hide rank");
    }

    #[test]
    fn null_space_spans_free_motion() {
        // x0 + x1 = c fixes one combination; x2 is untouched.
        let (r, n) = null_space(&[1.0, 1.0, 0.0], 1, 3);
        assert_eq!((r, n.len()), (1, 2));
        for v in &n {
            assert!((v[0] + v[1]).abs() < 1e-12);
        }
        // x2's own motion survives in the null space; x0 alone does not.
        assert!(n.iter().any(|v| v[2].abs() > 0.5));
        let (r, n) = null_space(&[], 0, 2);
        assert_eq!((r, n.len()), (0, 2));
    }

    #[test]
    fn minimum_norm_step() {
        // One equation x + y = 2 from (0,0): the minimum-norm solution is (1,1).
        let f = |x: &[f64], out: &mut Vec<f64>| out.push(x[0] + x[1] - 2.0);
        let o = solve(&f, &[0.0, 0.0], &[0, 1], &[1.0, 1.0], 1e-12, 50);
        assert!(o.converged);
        assert!((o.x[0] - 1.0).abs() < 1e-9 && (o.x[1] - 1.0).abs() < 1e-9);
        // Heavy weight on x keeps it in place.
        let o = solve(&f, &[0.0, 0.0], &[0, 1], &[1e6, 1.0], 1e-12, 50);
        assert!(o.x[0].abs() < 1e-5 && (o.x[1] - 2.0).abs() < 1e-5);
    }

    #[test]
    fn inconsistent_does_not_converge() {
        let f = |x: &[f64], out: &mut Vec<f64>| {
            out.push(x[0] - 1.0);
            out.push(x[0] - 2.0);
        };
        let o = solve(&f, &[0.0], &[0], &[1.0], 1e-10, 100);
        assert!(!o.converged);
        assert!(o.x[0].is_finite());
    }
}
