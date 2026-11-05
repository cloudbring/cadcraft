# cadcraft solver Lean development (ticket cloudbring/artcraft-algorithms#219): what is proved

Toolchain `leanprover/lean4:v4.24.0`, Mathlib `v4.24.0` (rev `f897ebcf72cd16f89ab4577d0c826cd14afaafc7`),
the same pin as `craft-libs` `crates/fft/lean` and `crates/blend/lean`. Run `./check.sh` (cache get, build,
axiom audit, regenerate vectors and `cmp`). `Audit.lean` prints the axioms of every theorem below;
`check.sh` fails on anything outside `propext`, `Classical.choice`, `Quot.sound`, or on any `sorry`.
There is no `sorry` and no `native_decide`. `./tamper.sh` proves the gates bite (see the end).

What the Rust code does (branch `algo/16-analytic-jacobian`, `crates/constraints`):

* `dof.rs::analyse`: per connected component, `dof = free parameters - rank J`,
  `redundant = rows - rank J`, `J` the analytic Jacobian in free-column coordinates (Fix locks
  columns, it adds no rows).
* `lm::rank_qr`: balance rows then columns to unit norm, Householder QR with column pivoting on the
  transpose, count pivots above `RANK_TOL = 1e-7` (relative) / `1e-12` (absolute).
* `solve_loop`: normal matrix `J W^-1 J^T + mu*diag_max*I`; dense Gaussian elimination up to 48 rows
  (`lm::dense_normal`), above that `sparse::solve_damped` (RCM order, banded Cholesky).

## Proved (CraftSolver/Dof.lean): DOF spec and invariance (any field `K`)

* `dof J = card cols - J.rank`, `redundant J = card rows - J.rank` (`Matrix.rank`).
* Permutations (the RCM ordering): `rank_reindex`, `dof_reindex`, `redundant_reindex` for any row and
  column bijections; `rank_permMatrix` for `P J Q` with `Equiv.Perm.permMatrix`.
* Scaling: `rank_diag_scale` (nonzero row and column factors); `rank_row_scale`, `rank_col_scale`
  (a factor may be anything on a zero row/column, which is what `balanced_rows` does: factor 0 on a
  zero row); over `ℝ`, `balRow` is the unit-norm row factor, `rank_balance_rows`,
  `rank_balance_cols`: the whole row-then-column balancing keeps the rank, hence DOF.
* Redundancy: `rank_dropRow_of_mem_span`: a row in the span of the others can be deleted without
  changing the rank (`dof_dropRow_of_mem_span`: nor DOF); `rank_dropRow_of_not_mem_span`: a row outside
  that span is independent, deleting it lowers the rank by exactly one. `finrank_span_insert_add_one`.

## Proved (CraftSolver/Elim.lean): the exact elimination is `Matrix.rank`

* `gsRank`: pivoted elimination over `ℚ`. Pivot = the row of largest squared norm (the solver's
  "largest remaining column norm"), stop when it is `0` (tolerance `0`), otherwise project the pivot
  direction out of the rest (`v - (<v,p>/<p,p>) p`, the Gram-Schmidt form of the Householder step,
  rational, no square root) and count.
* `gsRank_eq_rank`: `gsRank (rows of J) = J.rank` for every `J : Matrix (Fin m) (Fin n) ℚ`;
  core `gsRankF_eq` (fuel `>= length` gives `finrank (span rows)`), with `selMax_none`, `selMax_some`,
  `spanL_cons_proj`, `finrank_cons`. `dof_eq_gsRank`, `gsRank_perm` (reordering rows/columns gives the
  same count).
* `Generate.lean` runs this model (`rankOfRows`) to produce the vectors, and fails if `rank J != rank J^T`.

## Proved (CraftSolver/Solve.lean): dense solve versus RCM-ordered Cholesky, algebra only

* `reorder A σ` is `B_ij = A_{σ^-1 i, σ^-1 j}`, `reorderVec`: `y_i = b_{σ^-1 i}` (exactly the solver's
  `perm`). `reorder_eq_permMatrix`: `B = P A P^T`, `permMatrix_mulVec_eq`: `P x` is the reordered vector,
  `P = (σ⁻¹).permMatrix`.
* `solve_equiv`, `solve_equiv_permMatrix`: `A x = b  <->  (P A P^T)(P x) = P b`; `unreorder`: reading back
  `out[old] = z[perm[old]]` recovers `x`.
* `cholesky_solves`: if `B = L L^T`, `L y = c`, `L^T z = y` then `B z = c` (the forward and back
  substitution of the banded factor).
* `posDef_reorder`: reordering keeps a matrix positive definite. `spd_unique`: an SPD system has one solution.
  `dense_eq_ordered`: for SPD `A`, if `x` solves `A x = b` and `z` solves the reordered system then
  `z ∘ σ = x`: dense and ordered solves return the same `x`.
* `posDef_damped`: `J W^-1 J^T + c I` is positive definite for `W^-1 > 0` diagonal and `c > 0`
  (the damping is what makes the factorisation exist). `rank_normal`: the undamped `J W^-1 J^T` has
  `rank J` (so it is singular exactly when the rows are redundant or conflicting).

## NOT proved

* Floating point. Everything is exact. The solver's tolerances are not modelled: pivots are cut at
  `1e-7` relative / `1e-12` absolute, and in `balanced_rows` a column whose norm after row balancing is
  at most `COL_NOISE = 1e-6` is *zeroed* (not scaled). That last step is not a rank-preserving
  scaling; for an exact Jacobian it can lower the rank. Vector `tol_1e9` (exact rank 2) is pinned as a
  divergence: the solver reports 1. For analytic Jacobians the exact-zero entries are exactly zero, so
  the zeroing is never needed there; it only matters for numerical-difference noise.
* Householder QR itself is not formalised; the modelled elimination is the Gram-Schmidt
  equivalent, whose pivot norms equal the QR ones in exact arithmetic (stated, not proved).
* Banded structure: that the Cholesky factor of `P A P^T` stays within the RCM bandwidth, the RCM
  heuristic's quality, and `MAX_BAND`/`TooWide` fallbacks. Existence of the Cholesky factor of an SPD
  matrix is not reproved (`cholesky_solves` takes `B = L L^T` as a hypothesis).
* The Jacobian itself (analytic partials of each constraint kind). `sketch` vectors compare cadcraft's
  analytic Jacobian entry by entry with the exported rationals, so they pin it at those points only.
* Per-object freedom (`null_space` restricted to an object's columns), the Owen/biconnected split
  (`owen.rs` warm starts), spline end-frame numerical differentiation.

## Vectors (`tests/vectors/dof.csv`, 23 rows)

`case,source,rows,cols,matrix,rank,dof,redundant`, entries exact rationals.

* 7 `sketch`: Jacobians exported from real cadcraft sketches at exactly rational geometry
  (`rect_open`, `rect_fixed`, `rect_redundant`, `triangle_345`, `triangle_345_fixed`, `chain7_dist`,
  `parallel_perp_equal`). The literals in `Generate.lean` come from
  `cargo test -p cadcraft-constraints lean_vectors::export -- --ignored --nocapture`.
* 15 `synthetic`: identity, zero matrix, zero row, wide, tall, scaled duplicate rows, a 30-row distance
  chain, linear-congruential matrices with injected dependent rows up to 31x30, extreme (1e+-3) row
  and column scales, a gap-1e-3 matrix.
* 1 `tolerance` (`tol_1e9`).

## Rust checks (`crates/constraints/src/lean_vectors.rs`, `cargo test -p cadcraft-constraints`)

1. `rank_and_dof_match_exact_lean_values`: `lm::rank_qr` (the solver's tolerance) equals the exact rank
   on every non-`tolerance` row, and is strictly below it on `tol_1e9`.
2. `rank_is_invariant_under_permutation_and_scaling`: random row/column permutations and 1e-2..1e2
   row/column scalings leave `rank_qr` unchanged.
3. `sketch_jacobians_and_dof_match_cadcraft`: each sketch is rebuilt, its analytic Jacobian equals the CSV
   entry by entry (1e-12), and the public `dof()` returns the CSV `dof` and `redundant`.
4. `ordered_banded_solve_matches_dense_on_every_vector`: `sparse::solve_damped` (RCM + banded Cholesky) on
   randomly row-scrambled inputs against `lm::dense_normal`, `mu` in {1e-4, 1e-8}: relative difference
   at most 1e-10 for full-row-rank systems and 1e-7 for rank-deficient ones (measured worst 6.4e-9, on a
   system with 2 redundant rows at `mu = 1e-8`).

## Regenerating

```sh
export PATH="$HOME/.elan/bin:$PATH"
cd lean/solver && lake exe cache get && lake build
lake env lean Generate.lean > tests/vectors/dof.csv   # after a reviewed change only
./check.sh && ./tamper.sh
```
