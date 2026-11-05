import Mathlib
import CraftSolver.Dof

/-! Dense solve versus RCM-ordered banded Cholesky: the algebra.  See ../TRUST.md.

`sparse::solve_damped` forms `B = P A Pᵀ` (`A = J W⁻¹ Jᵀ + μ d I`) in the reverse Cuthill-McKee
order, writes `y[perm[old]] = rhs[old]`, factors `B = L Lᵀ`, solves `L Lᵀ z = y`, and returns
`out[old] = z[perm[old]]`.  Here `σ = perm`, `B i j = A (σ⁻¹ i) (σ⁻¹ j)`, `y i = b (σ⁻¹ i)`,
`x old = z (σ old)`.  Exact arithmetic only. -/

set_option linter.unusedSectionVars false

namespace CraftSolver

open Matrix

section reorder
variable {n : Type*} [Fintype n] [DecidableEq n] {R : Type*} [CommRing R]

/-- The reordered system matrix `B_{ij} = A_{σ⁻¹ i, σ⁻¹ j}` (= `P A Pᵀ`). -/
def reorder (A : Matrix n n R) (σ : Equiv.Perm n) : Matrix n n R := A.submatrix σ.symm σ.symm

/-- Reordered vector: `(P v)_i = v_{σ⁻¹ i}`. -/
def reorderVec (v : n → R) (σ : Equiv.Perm n) : n → R := fun i => v (σ.symm i)

theorem reorder_mulVec (A : Matrix n n R) (σ : Equiv.Perm n) (x : n → R) :
    reorder A σ *ᵥ reorderVec x σ = reorderVec (A *ᵥ x) σ := by
  ext i
  simp only [reorder, reorderVec, mulVec, dotProduct, submatrix_apply]
  exact Equiv.sum_comp σ.symm (fun j => A (σ.symm i) j * x j)

/-- (2) Solve equivalence: `A x = b` iff the reordered system holds for the reordered unknown. -/
theorem solve_equiv (A : Matrix n n R) (σ : Equiv.Perm n) (x b : n → R) :
    A *ᵥ x = b ↔ reorder A σ *ᵥ reorderVec x σ = reorderVec b σ := by
  rw [reorder_mulVec]
  constructor
  · intro h; rw [h]
  · intro h; ext i
    have := congrFun h (σ i)
    simpa [reorderVec] using this

/-- Undoing the reordering recovers the unknown. -/
theorem unreorder (x : n → R) (σ : Equiv.Perm n) :
    (fun old => reorderVec x σ (σ old)) = x := by
  ext i; simp [reorderVec]

/-- `P A Pᵀ` with `P = σ⁻¹.permMatrix` is the reordered matrix. -/
theorem reorder_eq_permMatrix (A : Matrix n n R) (σ : Equiv.Perm n) :
    reorder A σ = ((σ⁻¹).permMatrix R) * A * ((σ⁻¹).permMatrix R)ᵀ := by
  rw [transpose_permMatrix, inv_inv, Equiv.Perm.permMatrix, Equiv.Perm.permMatrix,
    PEquiv.toMatrix_toPEquiv_mul, PEquiv.mul_toMatrix_toPEquiv]
  rfl

/-- `P x` is the reordered vector. -/
theorem permMatrix_mulVec_eq (x : n → R) (σ : Equiv.Perm n) :
    ((σ⁻¹).permMatrix R) *ᵥ x = reorderVec x σ := by
  rw [Matrix.permMatrix_mulVec]; rfl

/-- Same statement in the textbook matrix form `(P A Pᵀ)(P x) = P b`. -/
theorem solve_equiv_permMatrix (A : Matrix n n R) (σ : Equiv.Perm n) (x b : n → R) :
    A *ᵥ x = b ↔
      (((σ⁻¹).permMatrix R) * A * ((σ⁻¹).permMatrix R)ᵀ) *ᵥ (((σ⁻¹).permMatrix R) *ᵥ x)
        = ((σ⁻¹).permMatrix R) *ᵥ b := by
  rw [← reorder_eq_permMatrix, permMatrix_mulVec_eq, permMatrix_mulVec_eq]
  exact solve_equiv A σ x b

/-- Algebraic core of any Cholesky solve: if `B = L Lᵀ`, `L y = c` and `Lᵀ z = y`, then `B z = c`.
(`L` need not even be triangular for this implication; the solver supplies a banded lower `L`.) -/
theorem cholesky_solves (B L : Matrix n n R) (hB : B = L * Lᵀ) (c y z : n → R)
    (h1 : L *ᵥ y = c) (h2 : Lᵀ *ᵥ z = y) : B *ᵥ z = c := by
  rw [hB, ← Matrix.mulVec_mulVec, h2, h1]

end reorder

section spd
variable {n : Type*} [Fintype n] [DecidableEq n]

/-- Reordering keeps positive definiteness (so the permuted matrix is still factorable). -/
theorem posDef_reorder {A : Matrix n n ℝ} (hA : A.PosDef) (σ : Equiv.Perm n) :
    (reorder A σ).PosDef := by
  refine ⟨hA.isHermitian.submatrix σ.symm, fun x hx => ?_⟩
  have hy : reorderVec (reorderVec x σ.symm) σ = x := by ext i; simp [reorderVec]
  have h := hA.2 (reorderVec x σ.symm) (by
    intro h0; apply hx; ext i
    have := congrFun h0 (σ.symm i)
    simpa [reorderVec] using this)
  have e : star x ⬝ᵥ (reorder A σ *ᵥ x) = star (reorderVec x σ.symm) ⬝ᵥ (A *ᵥ reorderVec x σ.symm) := by
    have := reorder_mulVec A σ (reorderVec x σ.symm)
    rw [hy] at this
    rw [this]
    simp only [dotProduct, reorderVec, Pi.star_apply, star_trivial]
    rw [← Equiv.sum_comp σ]
    simp
  rw [e]; exact h

/-- A positive definite matrix has a unique solution; so dense and reordered-banded solves, both
solving the same SPD system, return the same `x`. -/
theorem spd_unique {A : Matrix n n ℝ} (hA : A.PosDef) {x x' b : n → ℝ}
    (h : A *ᵥ x = b) (h' : A *ᵥ x' = b) : x = x' :=
  (Matrix.mulVec_injective_iff_isUnit.2 hA.isUnit) (h.trans h'.symm)

/-- Dense and RCM-ordered banded Cholesky agree: if `x` solves `A x = b` (dense) and `z` solves
the reordered system, then un-permuting `z` gives `x`. -/
theorem dense_eq_ordered {A : Matrix n n ℝ} (hA : A.PosDef) (σ : Equiv.Perm n) {x b : n → ℝ}
    {z : n → ℝ} (hx : A *ᵥ x = b) (hz : reorder A σ *ᵥ z = reorderVec b σ) :
    (fun old => z (σ old)) = x := by
  have hz' : reorder A σ *ᵥ reorderVec x σ = reorderVec b σ := (solve_equiv A σ x b).1 hx
  have hB := (posDef_reorder hA σ).isUnit
  have : z = reorderVec x σ := (Matrix.mulVec_injective_iff_isUnit.2 hB) (hz.trans hz'.symm)
  subst this; exact unreorder x σ

/-- Damped normal matrix `J W⁻¹ Jᵀ + c I` (`c = μ · diag_max`, `W⁻¹` positive diagonal) is
positive definite whenever `c > 0`: the damping is what makes the factorisation exist. -/
theorem posDef_damped {m k : Type*} [Fintype m] [DecidableEq m] [Fintype k] [DecidableEq k]
    (J : Matrix m k ℝ) (w : k → ℝ) (hw : ∀ j, 0 < w j) {c : ℝ} (hc : 0 < c) :
    (J * diagonal w * Jᵀ + c • (1 : Matrix m m ℝ)).PosDef := by
  have h1 : (J * diagonal w * Jᴴ).PosSemidef :=
    (Matrix.posSemidef_diagonal_iff.2 fun i => (hw i).le).mul_mul_conjTranspose_same J
  have h2 : (c • (1 : Matrix m m ℝ)).PosDef := by
    rw [Matrix.smul_one_eq_diagonal, Matrix.posDef_diagonal_iff]; intro; exact hc
  simpa using Matrix.PosDef.posSemidef_add h1 h2

/-- The undamped weighted normal matrix `J W⁻¹ Jᵀ` has the Jacobian's rank, so it is singular
exactly when the system is rank deficient (redundant or conflicting rows): the DOF diagnostic and
the factorisation's need for damping are the same fact. -/
theorem rank_normal {m k : Type*} [Fintype m] [DecidableEq m] [Fintype k] [DecidableEq k]
    (J : Matrix m k ℝ) (w : k → ℝ) (hw : ∀ j, 0 < w j) :
    (J * diagonal w * Jᵀ).rank = J.rank := by
  have hs : ∀ j, Real.sqrt (w j) ≠ 0 := fun j => (Real.sqrt_pos.2 (hw j)).ne'
  have e : J * diagonal w * Jᵀ = (J * diagonal (fun j => Real.sqrt (w j))) *
      (J * diagonal (fun j => Real.sqrt (w j)))ᵀ := by
    rw [transpose_mul, diagonal_transpose]
    simp only [Matrix.mul_assoc]
    rw [← Matrix.mul_assoc (diagonal _) (diagonal _), diagonal_mul_diagonal]
    congr 2
    ext i j
    simp [diagonal_apply, Real.mul_self_sqrt (hw _).le]
  rw [e, Matrix.rank_self_mul_transpose]
  have h2 : IsUnit (diagonal fun j => Real.sqrt (w j)).det := by
    rw [det_diagonal]; exact isUnit_iff_ne_zero.2 (Finset.prod_ne_zero_iff.2 fun j _ => hs j)
  exact Matrix.rank_mul_eq_left_of_isUnit_det _ _ h2

end spd

end CraftSolver
