import CraftSolver

open CraftSolver

/-! Exact Jacobian test vectors for the cadcraft rank / DOF routine.  See ../TRUST.md.

CSV columns: `case,source,rows,cols,matrix,rank,dof,redundant`; `matrix` is row-major, rows
separated by `;`, entries by spaces, each entry an exact rational `p` or `p/q`.
* `rank` is `CraftSolver.rankOfRows`, the exact-arithmetic pivoted elimination proved equal to
  `Matrix.rank` (`gsRank_eq_rank`); `dof = cols - rank`, `redundant = rows - rank`
  (`CraftSolver.dof`, `CraftSolver.redundant`).
* `sketch`: Jacobians exported from real cadcraft sketches by
  `cargo test -p cadcraft-constraints lean_vectors::export -- --ignored` (analytic Jacobian at
  exactly rational geometry, columns = free parameters).  The Rust test rebuilds each sketch and
  compares every entry.
* `synthetic`: built here (banded chains, linear-congruential matrices with injected dependent
  rows, extreme row/column scaling).
* `tolerance`: exact rank 2, but the solver's numerical tolerance reports 1 (pinned divergence). -/

abbrev Mat := List (List ℚ)

def sketches : List (String × String × Mat) := [
  ("rect_open", "sketch", [
    [0, 0, 1, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 1, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 1, 0, -1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 1, 0, -1, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, -1, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, -1, 0, 0],
    [-1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0],
    [0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    [0, 1, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, -1, 0, 0, 0, 0],
    [0, 0, 0, 0, 1, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, -1, 0],
  ]),
  ("rect_fixed", "sketch", [
    [1, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 1, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 1, 0, -1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 1, 0, -1, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 0, -1, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, -1, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    [0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 1, 0, -1, 0, 0, 0, 0],
    [0, 0, 1, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, -1, 0],
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, -1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
  ]),
  ("rect_redundant", "sketch", [
    [1, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 1, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 1, 0, -1, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 1, 0, -1, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 1, 0, -1, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, -1, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    [0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 1, 0, -1, 0, 0, 0, 0],
    [0, 0, 1, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, -1, 0],
    [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, -1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, -1],
  ]),
  ("triangle_345", "sketch", [
    [-1, 0, 1, 0, 0, 0],
    [0, 0, 0, -1, 0, 1],
    [-4/5, -3/5, 0, 0, 4/5, 3/5],
  ]),
  ("triangle_345_fixed", "sketch", [
    [1, 0, 0, 0],
    [0, -1, 0, 1],
    [0, 0, 4/5, 3/5],
    [0, -1, 0, 0],
  ]),
  ("chain7_dist", "sketch", [
    [-3/5, -4/5, 3/5, 4/5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, -3/5, -4/5, 3/5, 4/5, 0, 0, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, -3/5, -4/5, 3/5, 4/5, 0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, -3/5, -4/5, 3/5, 4/5, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, -3/5, -4/5, 3/5, 4/5, 0, 0],
    [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, -3/5, -4/5, 3/5, 4/5],
  ]),
  ("parallel_perp_equal", "sketch", [
    [0, 1/4, 0, -1/4, 0, -1/4, 0, 1/4, 0, 0, 0, 0],
    [0, -1/4, 0, 1/4, 0, 0, 0, 0, -1/2, 0, 1/2, 0],
    [-1, 0, 1, 0, 1, 0, -1, 0, 0, 0, 0, 0],
  ]),
]

/-- Linear congruential generator (small integers). -/
def lcg (s : Nat) : Nat := (s * 1103515245 + 12345) % 2147483648

def lcgMat (seed r c : Nat) : Mat := Id.run do
  let mut s := seed
  let mut m : Mat := []
  for _ in [0:r] do
    let mut row : List ℚ := []
    for _ in [0:c] do
      s := lcg s
      row := row ++ [((s / 65536 % 7 : Nat) : ℚ) - 3]
    m := m ++ [row]
  return m

def addRows (a b : List ℚ) (x y : ℚ) : List ℚ := List.zipWith (fun u v => x * u + y * v) a b

/-- `base` followed by `k` rows that are combinations of earlier rows (redundant constraints). -/
def withDependent (base : Mat) (k : Nat) : Mat := Id.run do
  let mut m := base
  for i in [0:k] do
    let a := m.getD (i % base.length) []
    let b := m.getD ((3 * i + 1) % base.length) []
    m := m ++ [addRows a b ((i : ℚ) + 1) (-2)]
  return m

/-- Distance chain: row `i` has `(-3,-4,3,4)/5` in columns `2i..2i+3`. -/
def chainMat (k : Nat) : Mat :=
  (List.range k).map fun i => (List.range (2 * k + 2)).map fun j =>
    if j == 2 * i then -3 / 5 else if j == 2 * i + 1 then -4 / 5
    else if j == 2 * i + 2 then 3 / 5 else if j == 2 * i + 3 then 4 / 5 else 0

def identity (n : Nat) : Mat := (List.range n).map fun i => (List.range n).map fun j => if i == j then 1 else 0

def scaleRC (m : Mat) (rs cs : List ℚ) : Mat :=
  (m.zip rs).map fun (row, r) => (row.zip cs).map fun (x, c) => r * c * x

def synthetic : List (String × String × Mat) :=
  let b1 := lcgMat 7 8 12
  let b2 := lcgMat 11 20 24
  let b3 := lcgMat 23 24 30
  let sp := (chainMat 20).map fun row => row ++ [0, 0, 0]
  [ ("identity_5", "synthetic", identity 5),
    ("zero_3x4", "synthetic", [[0, 0, 0, 0], [0, 0, 0, 0], [0, 0, 0, 0]]),
    ("zero_row", "synthetic", [[1, 2, 3], [0, 0, 0], [4, 5, 6]]),
    ("wide_2x5", "synthetic", [[1, 0, 2, 0, 3], [0, 1, 0, 4, 0]]),
    ("tall_5x2", "synthetic", [[1, 0], [0, 1], [1, 1], [2, 3], [5, -7]]),
    ("dup_scaled_rows", "synthetic", [[1, 2, 3, 4], [2, 4, 6, 8], [0, 1, 0, 1], [1, 3, 3, 5]]),
    ("chain_dist_30", "synthetic", chainMat 30),
    ("chain_dist_20_dup", "synthetic", withDependent sp 6),
    ("lcg_8x12", "synthetic", b1),
    ("lcg_8x12_dep2", "synthetic", withDependent b1 2),
    ("lcg_20x24_dep5", "synthetic", withDependent b2 5),
    ("lcg_24x30_dep7", "synthetic", withDependent b3 7),
    ("scaled_chain_8", "synthetic",
      scaleRC (chainMat 8) [1000, 1/1000, 1, 10, 1/10, 100, 1/100, 1] (List.replicate 18 1 ++ [])),
    ("scaled_lcg_8x12", "synthetic",
      scaleRC b1 [1000, 1/1000, 1, 10, 1/10, 100, 1/100, 1]
        [1, 1/10, 10, 1, 1/10, 10, 1, 1/10, 10, 1, 1/10, 10]),
    ("near_gap_1e3", "synthetic", [[1, 0], [1, 1/1000]]),
    ("tol_1e9", "tolerance", [[1, 0], [1, 1/1000000000]]) ]

def fmtRow (r : List ℚ) : String := " ".intercalate (r.map toString)

def line (c : String × String × Mat) : String :=
  let (name, src, m) := c
  let rows := m.length
  let cols := (m.headD []).length
  if m.any (fun r => r.length != cols) then panic! s!"ragged matrix {name}" else
  let rank := rankOfRows cols m
  -- Independent cross-check: the transpose has the same rank (proved: `Matrix.rank_transpose`).
  let tr : Mat := (List.range cols).map fun j => m.map fun r => r.getD j 0
  if rankOfRows rows tr != rank then panic! s!"rank(J) != rank(J^T) for {name}" else
  s!"{name},{src},{rows},{cols},{";".intercalate (m.map fmtRow)},{rank},{cols - rank},{rows - rank}"

def generate : IO Unit := do
  IO.println "case,source,rows,cols,matrix,rank,dof,redundant"
  for c in sketches ++ synthetic do
    IO.println (line c)

#eval generate
