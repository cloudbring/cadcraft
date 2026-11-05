import Mathlib

/-! Degrees of freedom of a constraint system from the rank of its Jacobian.  See ../TRUST.md.

cadcraft (`crates/constraints/src/dof.rs`, `analyse`): per connected component the remaining DOF is
`free parameters - rank J`, and `redundant = rows - rank J`.  Here `J : Matrix m n K` over a field. -/

namespace CraftSolver

open Matrix

variable {K : Type*} [Field K]

/-- Degrees of freedom: number of free parameters (columns) minus the rank of the Jacobian. -/
noncomputable def dof {m n : Type*} [Fintype m] [Fintype n] (J : Matrix m n K) : ℕ :=
  Fintype.card n - J.rank

/-- Equations beyond the rank (the solver's `redundant`). -/
noncomputable def redundant {m n : Type*} [Fintype m] [Fintype n] (J : Matrix m n K) : ℕ :=
  Fintype.card m - J.rank

/-! ### Rank invariance under permutations (the RCM ordering) -/

/-- Re-indexing rows and columns by bijections (any reordering of the system) keeps the rank. -/
theorem rank_reindex {m n m' n' : Type*} [Fintype m] [Fintype n] [Fintype m'] [Fintype n']
    [DecidableEq m] [DecidableEq n] [DecidableEq m'] [DecidableEq n']
    (J : Matrix m n K) (e : m' ≃ m) (f : n' ≃ n) : (J.submatrix e f).rank = J.rank := by
  exact Matrix.rank_submatrix J e f

/-- DOF is unchanged by reordering rows and columns. -/
theorem dof_reindex {m n m' n' : Type*} [Fintype m] [Fintype n] [Fintype m'] [Fintype n']
    [DecidableEq m] [DecidableEq n] [DecidableEq m'] [DecidableEq n']
    (J : Matrix m n K) (e : m' ≃ m) (f : n' ≃ n) : dof (J.submatrix e f) = dof J := by
  unfold dof
  rw [rank_reindex, Fintype.card_congr f]

/-- Redundancy count is unchanged by reordering rows and columns. -/
theorem redundant_reindex {m n m' n' : Type*} [Fintype m] [Fintype n] [Fintype m'] [Fintype n']
    [DecidableEq m] [DecidableEq n] [DecidableEq m'] [DecidableEq n']
    (J : Matrix m n K) (e : m' ≃ m) (f : n' ≃ n) : redundant (J.submatrix e f) = redundant J := by
  unfold redundant
  rw [rank_reindex, Fintype.card_congr e]

/-- The same statement in the matrix form `P J Qᵀ` with permutation matrices. -/
theorem rank_permMatrix {m n : Type*} [Fintype m] [Fintype n] [DecidableEq m] [DecidableEq n]
    (J : Matrix m n K) (σ : Equiv.Perm m) (τ : Equiv.Perm n) :
    (σ.permMatrix K * J * τ.permMatrix K).rank = J.rank := by
  have hσ : IsUnit (σ.permMatrix K).det := by
    rw [Matrix.det_permutation]
    rcases Int.units_eq_one_or (Equiv.Perm.sign σ) with h | h <;> simp [h]
  have hτ : IsUnit (τ.permMatrix K).det := by
    rw [Matrix.det_permutation]
    rcases Int.units_eq_one_or (Equiv.Perm.sign τ) with h | h <;> simp [h]
  rw [Matrix.rank_mul_eq_left_of_isUnit_det _ _ hτ, Matrix.rank_mul_eq_right_of_isUnit_det _ _ hσ]

/-! ### Rank invariance under the solver's scaling -/

/-- Scaling rows and columns by nonzero factors keeps the rank. -/
theorem rank_diag_scale {m n : Type*} [Fintype m] [Fintype n] [DecidableEq m] [DecidableEq n]
    (J : Matrix m n K) (r : m → K) (c : n → K) (hr : ∀ i, r i ≠ 0) (hc : ∀ j, c j ≠ 0) :
    (diagonal r * J * diagonal c).rank = J.rank := by
  have h1 : IsUnit (diagonal r).det := by
    rw [det_diagonal]; exact (isUnit_iff_ne_zero.2 (Finset.prod_ne_zero_iff.2 fun i _ => hr i))
  have h2 : IsUnit (diagonal c).det := by
    rw [det_diagonal]; exact (isUnit_iff_ne_zero.2 (Finset.prod_ne_zero_iff.2 fun j _ => hc j))
  rw [Matrix.rank_mul_eq_left_of_isUnit_det _ _ h2, Matrix.rank_mul_eq_right_of_isUnit_det _ _ h1]

/-- The solver's balancing also maps zero rows to zero (factor `0`).  A row factor may be any
value as long as it is nonzero on every nonzero row. -/
theorem rank_row_scale {m n : Type*} [Fintype m] [Fintype n] [DecidableEq m] [DecidableEq n]
    (J : Matrix m n K) (r : m → K) (hr : ∀ i, r i ≠ 0 ∨ ∀ j, J i j = 0) :
    (diagonal r * J).rank = J.rank := by
  classical
  let r' : m → K := fun i => if r i = 0 then 1 else r i
  have hr' : ∀ i, r' i ≠ 0 := fun i => by by_cases h : r i = 0 <;> simp [r', h]
  have heq : diagonal r * J = diagonal r' * J := by
    ext i j
    rw [diagonal_mul, diagonal_mul]
    rcases hr i with h | h
    · simp [r', h]
    · simp [h]
  have h1 : IsUnit (diagonal r').det := by
    rw [det_diagonal]; exact (isUnit_iff_ne_zero.2 (Finset.prod_ne_zero_iff.2 fun i _ => hr' i))
  rw [heq, Matrix.rank_mul_eq_right_of_isUnit_det _ _ h1]

/-- Column version of [`rank_row_scale`]. -/
theorem rank_col_scale {m n : Type*} [Fintype m] [Fintype n] [DecidableEq m] [DecidableEq n]
    (J : Matrix m n K) (c : n → K) (hc : ∀ j, c j ≠ 0 ∨ ∀ i, J i j = 0) :
    (J * diagonal c).rank = J.rank := by
  have h := rank_row_scale Jᵀ c (fun j => by simpa [transpose_apply] using hc j)
  rw [← Matrix.rank_transpose (J * diagonal c), transpose_mul, diagonal_transpose, h,
    Matrix.rank_transpose]

/-- The solver's balancing (`lm::balanced_rows`: rows then columns scaled to unit norm, zero ones
left alone) in exact arithmetic.  Over `ℝ`. -/
noncomputable def balRow {m n : Type*} [Fintype n] (J : Matrix m n ℝ) (i : m) : ℝ :=
  (Real.sqrt (∑ j, J i j ^ 2))⁻¹

theorem balRow_spec {m n : Type*} [Fintype n] (J : Matrix m n ℝ) (i : m) :
    balRow J i ≠ 0 ∨ ∀ j, J i j = 0 := by
  unfold balRow
  by_cases h : ∑ j, J i j ^ 2 = 0
  · right; intro j
    have := (Finset.sum_eq_zero_iff_of_nonneg (fun j _ => sq_nonneg (J i j))).1 h j (Finset.mem_univ _)
    exact pow_eq_zero_iff (two_ne_zero) |>.1 this
  · left; exact inv_ne_zero (Real.sqrt_ne_zero'.2 (lt_of_le_of_ne (Finset.sum_nonneg fun j _ => sq_nonneg _) (Ne.symm h)))

/-- Row balancing preserves the rank, hence DOF. -/
theorem rank_balance_rows {m n : Type*} [Fintype m] [Fintype n] [DecidableEq m] [DecidableEq n]
    (J : Matrix m n ℝ) : (diagonal (balRow J) * J).rank = J.rank :=
  rank_row_scale J _ (balRow_spec J)

/-- Column balancing (applied after row balancing) preserves the rank, hence DOF. -/
theorem rank_balance_cols {m n : Type*} [Fintype m] [Fintype n] [DecidableEq m] [DecidableEq n]
    (J : Matrix m n ℝ) :
    ((diagonal (balRow J) * J) * diagonal (balRow (diagonal (balRow J) * J)ᵀ)).rank = J.rank := by
  rw [rank_col_scale _ _ (fun j => by
    have := balRow_spec (diagonal (balRow J) * J)ᵀ j
    simpa [transpose_apply] using this), rank_balance_rows]

/-! ### Redundant constraints -/

/-- Adjoining a vector outside the span raises the dimension by exactly one. -/
theorem finrank_span_insert_add_one {V : Type*} [AddCommGroup V] [Module K V] [FiniteDimensional K V]
    {x : V} {s : Set V} (h : x ∉ Submodule.span K s) :
    Module.finrank K (Submodule.span K (insert x s)) = Module.finrank K (Submodule.span K s) + 1 := by
  have hx : x ≠ 0 := fun h0 => h (h0 ▸ Submodule.zero_mem _)
  have hd : Disjoint (Submodule.span K s) (K ∙ x) := (Submodule.disjoint_span_singleton' hx).2 h
  have := Submodule.finrank_sup_add_finrank_inf_eq (Submodule.span K s) (K ∙ x)
  rw [hd.eq_bot, finrank_bot, add_zero, finrank_span_singleton hx] at this
  rw [Submodule.span_insert, sup_comm]
  omega

/-- Delete row `i`. -/
def dropRow {m n : Type*} (J : Matrix m n K) (i : m) : Matrix {k // k ≠ i} n K :=
  J.submatrix (fun k => k.1) id

/-- A constraint row lying in the span of the others is redundant: deleting it leaves the rank
unchanged (so also DOF). -/
theorem rank_dropRow_of_mem_span {m n : Type*} [Fintype m] [Fintype n] [DecidableEq m]
    (J : Matrix m n K) (i : m)
    (h : J.row i ∈ Submodule.span K (Set.range (dropRow J i).row)) :
    (dropRow J i).rank = J.rank := by
  classical
  have hr : Set.range J.row = insert (J.row i) (Set.range (dropRow J i).row) := by
    ext v; constructor
    · rintro ⟨k, rfl⟩
      by_cases hk : k = i
      · subst hk; exact Or.inl rfl
      · exact Or.inr ⟨⟨k, hk⟩, rfl⟩
    · rintro (rfl | ⟨k, rfl⟩)
      · exact ⟨i, rfl⟩
      · exact ⟨k.1, rfl⟩
  rw [Matrix.rank_eq_finrank_span_row, Matrix.rank_eq_finrank_span_row, hr,
    Submodule.span_insert_eq_span h]

/-- Converse: a row NOT in the span of the others is an independent constraint; deleting it
lowers the rank by exactly one. -/
theorem rank_dropRow_of_not_mem_span {m n : Type*} [Fintype m] [Fintype n] [DecidableEq m]
    (J : Matrix m n K) (i : m)
    (h : J.row i ∉ Submodule.span K (Set.range (dropRow J i).row)) :
    (dropRow J i).rank + 1 = J.rank := by
  classical
  have hr : Set.range J.row = insert (J.row i) (Set.range (dropRow J i).row) := by
    ext v; constructor
    · rintro ⟨k, rfl⟩
      by_cases hk : k = i
      · subst hk; exact Or.inl rfl
      · exact Or.inr ⟨⟨k, hk⟩, rfl⟩
    · rintro (rfl | ⟨k, rfl⟩)
      · exact ⟨i, rfl⟩
      · exact ⟨k.1, rfl⟩
  rw [Matrix.rank_eq_finrank_span_row, Matrix.rank_eq_finrank_span_row, hr]
  exact (finrank_span_insert_add_one h).symm

/-- Removing a redundant row leaves DOF unchanged and lowers the redundancy count by one. -/
theorem dof_dropRow_of_mem_span {m n : Type*} [Fintype m] [Fintype n] [DecidableEq m]
    (J : Matrix m n K) (i : m)
    (h : J.row i ∈ Submodule.span K (Set.range (dropRow J i).row)) :
    dof (dropRow J i) = dof J := by
  unfold dof; rw [rank_dropRow_of_mem_span J i h]

end CraftSolver
