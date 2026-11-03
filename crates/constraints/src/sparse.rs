//! Envelope Cholesky for the damped normal matrix `J W⁻¹ Jᵀ + μ I`.
//!
//! `faer` stays out of the crate. The KittyCAD `ezpz` stack was set aside because it pulls `faer`
//! in to model KCL, not this drawing database (see the crate docs). Small systems stay on the
//! dense Gaussian elimination in [`crate::lm`], which keeps those steps bit-for-bit. Larger ones
//! are ordered with reverse Cuthill–McKee and factored as a banded lower triangle. A normal
//! matrix whose envelope is wider than [`MAX_BAND`] is refused rather than filled in densely.
//!
//! This is not a multifrontal supernodal factor and it does not do delayed pivoting. Damping is
//! what keeps the shift positive definite.

use std::collections::VecDeque;

/// Widest `|i − j|` stored in the envelope. A chain of point distances has bandwidth 1 after
/// ordering; a sketch whose constraints all share parameters will not.
pub(crate) const MAX_BAND: usize = 256;

/// Most entries in one banded factor (`n * (band + 1)`).
const MAX_ENVELOPE: usize = 4_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FactorError {
    /// A pivot was not positive. The caller raises `μ` and tries again.
    Singular,
    /// The ordered envelope does not fit [`MAX_BAND`] / [`MAX_ENVELOPE`].
    TooWide,
}

/// Solve `(J W⁻¹ Jᵀ + μ D I) y = rhs`. `trips` is `(row, column, value)` of `J` with columns
/// into `winv`. `mu` is multiplied by the largest undamped diagonal, matching the dense solver.
pub(crate) fn solve_damped(n: usize, trips: &[(u32, u32, f64)], winv: &[f64], rhs: &[f64], mu: f64) -> Result<Vec<f64>, FactorError> {
    if n == 0 {
        return Ok(Vec::new());
    }
    let cols = columns(trips, winv.len());
    if cols.iter().any(|c| c.len() > MAX_BAND + 1) {
        return Err(FactorError::TooWide);
    }
    let graph = row_graph(&cols, n);
    let perm = rcm(&graph);
    let band = bandwidth(&graph, &perm);
    if band > MAX_BAND || n.saturating_mul(band.saturating_add(1)) > MAX_ENVELOPE {
        return Err(FactorError::TooWide);
    }
    let diag = diag_max(&cols, winv, n);
    let mut band_m = scatter(&cols, winv, &perm, n, band);
    let shift = mu * diag;
    for i in 0..n {
        if let Some(v) = band_m.get_mut(i * (band + 1)) {
            *v += shift;
        }
    }
    factor(&mut band_m, n, band)?;
    let mut y = vec![0.0; n];
    for (old, &rhs_i) in rhs.iter().enumerate().take(n) {
        if let Some(&p) = perm.get(old)
            && let Some(slot) = y.get_mut(p)
        {
            *slot = rhs_i;
        }
    }
    triangular(&band_m, n, band, &mut y);
    let mut out = vec![0.0; n];
    for (old, &p) in perm.iter().enumerate() {
        if let Some(&v) = y.get(p)
            && let Some(slot) = out.get_mut(old)
        {
            *slot = v;
        }
    }
    out.iter().all(|v| v.is_finite()).then_some(out).ok_or(FactorError::Singular)
}

fn columns(trips: &[(u32, u32, f64)], n_cols: usize) -> Vec<Vec<(u32, f64)>> {
    let mut cols = vec![Vec::new(); n_cols];
    for &(r, c, v) in trips {
        if let Some(slot) = cols.get_mut(c as usize) {
            slot.push((r, v));
        }
    }
    cols
}

fn diag_max(cols: &[Vec<(u32, f64)>], winv: &[f64], n: usize) -> f64 {
    let mut diag = vec![0.0; n];
    for (c, entries) in cols.iter().enumerate() {
        let w = winv.get(c).copied().unwrap_or(0.0);
        for &(i, v) in entries {
            if let Some(d) = diag.get_mut(i as usize) {
                *d += v * v * w;
            }
        }
    }
    diag.into_iter().fold(0.0_f64, f64::max).max(1e-12)
}

fn row_graph(cols: &[Vec<(u32, f64)>], n: usize) -> Vec<Vec<u32>> {
    let mut g = vec![Vec::new(); n];
    for entries in cols {
        for i in 0..entries.len() {
            for k in i + 1..entries.len() {
                let (Some(&(a, _)), Some(&(b, _))) = (entries.get(i), entries.get(k)) else { continue };
                if a != b {
                    if let (Some(na), Some(nb)) = (g.get_mut(a as usize), Some(b)) {
                        na.push(nb);
                    }
                    if let Some(nb) = g.get_mut(b as usize) {
                        nb.push(a);
                    }
                }
            }
        }
    }
    for nbrs in &mut g {
        nbrs.sort_unstable();
        nbrs.dedup();
    }
    g
}

fn bandwidth(graph: &[Vec<u32>], perm: &[usize]) -> usize {
    let mut band = 0usize;
    for (i, nbrs) in graph.iter().enumerate() {
        let Some(&pi) = perm.get(i) else { continue };
        for &j in nbrs {
            if let Some(&pj) = perm.get(j as usize) {
                band = band.max(pi.abs_diff(pj));
            }
        }
    }
    band
}

fn scatter(cols: &[Vec<(u32, f64)>], winv: &[f64], perm: &[usize], n: usize, band: usize) -> Vec<f64> {
    let mut a = vec![0.0; n.saturating_mul(band + 1)];
    let w = band + 1;
    for (c, entries) in cols.iter().enumerate() {
        let weight = winv.get(c).copied().unwrap_or(0.0);
        for &(ri, vi) in entries {
            for &(rk, vk) in entries {
                let (Some(&pi), Some(&pk)) = (perm.get(ri as usize), perm.get(rk as usize)) else { continue };
                if pi >= pk
                    && pi - pk <= band
                    && let Some(slot) = a.get_mut(pi * w + (pi - pk))
                {
                    *slot += vi * weight * vk;
                }
            }
        }
    }
    a
}

/// In-place lower Cholesky. Diagonal stores `L_ii`; below it, `L_ij` at offset `i - j`.
fn factor(a: &mut [f64], n: usize, band: usize) -> Result<(), FactorError> {
    let w = band + 1;
    for i in 0..n {
        let mut diag = a.get(i * w).copied().unwrap_or(0.0);
        for k in i.saturating_sub(band)..i {
            let lik = a.get(i * w + (i - k)).copied().unwrap_or(0.0);
            diag -= lik * lik;
        }
        if diag <= 0.0 || !diag.is_finite() {
            return Err(FactorError::Singular);
        }
        let lii = diag.sqrt();
        if let Some(slot) = a.get_mut(i * w) {
            *slot = lii;
        }
        for j in i + 1..n.min(i + band + 1) {
            let mut sum = a.get(j * w + (j - i)).copied().unwrap_or(0.0);
            let k0 = j.saturating_sub(band);
            for k in k0..i {
                let ljk = a.get(j * w + (j - k)).copied().unwrap_or(0.0);
                let lik = a.get(i * w + (i - k)).copied().unwrap_or(0.0);
                sum -= ljk * lik;
            }
            if let Some(slot) = a.get_mut(j * w + (j - i)) {
                *slot = sum / lii;
            }
        }
    }
    Ok(())
}

fn triangular(a: &[f64], n: usize, band: usize, y: &mut [f64]) {
    let w = band + 1;
    for i in 0..n {
        let mut sum = y.get(i).copied().unwrap_or(0.0);
        for k in i.saturating_sub(band)..i {
            sum -= a.get(i * w + (i - k)).copied().unwrap_or(0.0) * y.get(k).copied().unwrap_or(0.0);
        }
        if let Some(slot) = y.get_mut(i) {
            *slot = sum / a.get(i * w).copied().unwrap_or(1.0);
        }
    }
    for i in (0..n).rev() {
        let mut sum = y.get(i).copied().unwrap_or(0.0);
        for j in i + 1..n.min(i + band + 1) {
            sum -= a.get(j * w + (j - i)).copied().unwrap_or(0.0) * y.get(j).copied().unwrap_or(0.0);
        }
        if let Some(slot) = y.get_mut(i) {
            *slot = sum / a.get(i * w).copied().unwrap_or(1.0);
        }
    }
}

fn rcm(graph: &[Vec<u32>]) -> Vec<usize> {
    let n = graph.len();
    let mut seen = vec![false; n];
    let mut perm = vec![0; n];
    let mut at = 0usize;
    let mut seeds: Vec<usize> = (0..n).collect();
    seeds.sort_by_key(|&i| graph.get(i).map_or(0, Vec::len));
    for seed in seeds {
        if seen.get(seed).copied().unwrap_or(true) {
            continue;
        }
        let start = peripheral(graph, seed, &seen);
        let mut comp = Vec::new();
        cm_visit(graph, start, &mut seen, &mut comp);
        for old in comp.iter().rev() {
            if let Some(slot) = perm.get_mut(*old) {
                *slot = at;
            }
            at += 1;
        }
    }
    perm
}

fn peripheral(graph: &[Vec<u32>], seed: usize, seen: &[bool]) -> usize {
    let end = bfs_last(graph, seed, seen);
    bfs_last(graph, end, seen)
}

fn bfs_last(graph: &[Vec<u32>], start: usize, seen: &[bool]) -> usize {
    let mut vis = seen.to_vec();
    if let Some(slot) = vis.get_mut(start) {
        *slot = true;
    }
    let mut q = VecDeque::from([start]);
    let mut last = start;
    while let Some(v) = q.pop_front() {
        last = v;
        let Some(nbrs) = graph.get(v) else { continue };
        let mut next: Vec<u32> = nbrs.iter().copied().filter(|&u| vis.get(u as usize).copied() != Some(true)).collect();
        next.sort_unstable();
        for u in next {
            if let Some(slot) = vis.get_mut(u as usize) {
                *slot = true;
                q.push_back(u as usize);
            }
        }
    }
    last
}

fn cm_visit(graph: &[Vec<u32>], start: usize, seen: &mut [bool], order: &mut Vec<usize>) {
    if let Some(slot) = seen.get_mut(start) {
        *slot = true;
    }
    let mut q = VecDeque::from([start]);
    while let Some(v) = q.pop_front() {
        order.push(v);
        let Some(nbrs) = graph.get(v) else { continue };
        let mut next: Vec<usize> = nbrs.iter().map(|&u| u as usize).filter(|&u| seen.get(u).copied() != Some(true)).collect();
        next.sort_by_key(|&u| graph.get(u).map_or(0, Vec::len));
        for u in next {
            if seen.get(u).copied() == Some(true) {
                continue;
            }
            if let Some(slot) = seen.get_mut(u) {
                *slot = true;
            }
            q.push_back(u);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dense_spd(n: usize, trips: &[(u32, u32, f64)], winv: &[f64], mu: f64) -> Vec<f64> {
        let mut a = vec![0.0; n * n];
        for (c, &w) in winv.iter().enumerate() {
            let mut col = vec![0.0; n];
            for &(r, k, v) in trips {
                if k as usize == c
                    && let Some(slot) = col.get_mut(r as usize)
                {
                    *slot = v;
                }
            }
            for i in 0..n {
                for k in 0..n {
                    a[i * n + k] += col[i] * w * col[k];
                }
            }
        }
        let diag = (0..n).map(|i| a[i * n + i]).fold(0.0_f64, f64::max).max(1e-12);
        for i in 0..n {
            a[i * n + i] += mu * diag;
        }
        a
    }

    #[test]
    fn banded_factor_matches_a_diagonal_and_a_chain() {
        let n = 8;
        let trips: Vec<(u32, u32, f64)> = (0..n).map(|i| (i as u32, i as u32, 2.0 + i as f64 * 0.1)).collect();
        let winv = vec![1.0; n];
        let rhs: Vec<f64> = (0..n).map(|i| i as f64 - 3.0).collect();
        let y = solve_damped(n, &trips, &winv, &rhs, 1e-8).unwrap();
        let a = dense_spd(n, &trips, &winv, 1e-8);
        for i in 0..n {
            let mut s = 0.0;
            for k in 0..n {
                s += a[i * n + k] * y[k];
            }
            assert!((s - rhs[i]).abs() < 1e-8, "diag row {i}: {s} vs {}", rhs[i]);
        }
        // Neighbouring residuals share a parameter: bandwidth 1.
        let mut chain = Vec::new();
        for i in 0..n - 1 {
            chain.push((i as u32, i as u32, 1.0));
            chain.push((i as u32, i as u32 + 1, -1.0));
        }
        let winv = vec![1.0; n];
        let y = solve_damped(n - 1, &chain, &winv, &rhs[..n - 1], 1e-6).unwrap();
        let a = dense_spd(n - 1, &chain, &winv, 1e-6);
        for i in 0..n - 1 {
            let mut s = 0.0;
            for k in 0..n - 1 {
                s += a[i * (n - 1) + k] * y[k];
            }
            assert!((s - rhs[i]).abs() < 1e-8, "chain row {i}: {s} vs {}", rhs[i]);
        }
    }

    #[test]
    fn a_dense_clique_past_the_band_is_too_wide() {
        let n = MAX_BAND + 3;
        let trips: Vec<(u32, u32, f64)> = (0..n).map(|i| (i as u32, 0, 1.0)).collect();
        let err = solve_damped(n, &trips, &[1.0], &vec![1.0; n], 1e-6);
        assert!(matches!(err, Err(FactorError::TooWide)));
    }
}
