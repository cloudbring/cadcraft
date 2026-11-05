import Mathlib
import CraftSolver.Dof

/-! Exact-arithmetic model of cadcraft's rank routine.  See ../TRUST.md.

`lm::rank_qr` is a Householder QR with column pivoting on the transpose of the (balanced)
Jacobian: at each step it picks the column (= Jacobian row) of largest remaining norm, stops when
that norm is `<=` a tolerance, otherwise eliminates the pivot direction from the rest.  Its rank is
the number of steps taken.  In exact arithmetic with tolerance `0`, "eliminate the pivot direction
by a Householder reflection" has the same effect on the remaining norms as Gram-Schmidt projection,
which stays rational (no square root): `v ↦ v - (⟨v,p⟩/⟨p,p⟩) p`.  That is what is modelled and
proved equal to `Matrix.rank` over `ℚ`. -/

namespace CraftSolver

open Matrix

abbrev Vec (n : ℕ) := Fin n → ℚ

/-- Pivot: a vector of largest squared norm and the others; `none` when everything is zero
(the solver's "largest remaining column norm drops to the tolerance", tolerance `0`). -/
def selMax {n : ℕ} (L : List (Vec n)) : Option (Vec n × List (Vec n)) :=
  match L.argmax (fun v => v ⬝ᵥ v) with
  | none => none
  | some p => if p ⬝ᵥ p = 0 then none else some (p, L.erase p)

/-- Remove the pivot direction from `v`. -/
def proj {n : ℕ} (p v : Vec n) : Vec n :=
  let c := (v ⬝ᵥ p) / (p ⬝ᵥ p)  -- hoisted so the interpreter evaluates it once per step
  fun j => v j - c * p j

theorem proj_eq {n : ℕ} (p v : Vec n) : proj p v = v - ((v ⬝ᵥ p) / (p ⬝ᵥ p)) • p := by
  ext j; simp [proj]

/-- A vector backed by an array.  Used to materialise projections: the compiler eta-expands any
definition returning `Vec n`, so without an `Array` in between every entry would re-evaluate the
whole chain of earlier projections (exponential in depth).  `ofArr (Array.ofFn v) = v`
(`ofArr_ofFn`), so it changes nothing semantically. -/
@[noinline] def ofArr {n : ℕ} (a : Array ℚ) : Vec n := fun j => a.getD j.1 0

theorem ofArr_ofFn {n : ℕ} (v : Vec n) : (ofArr (Array.ofFn v) : Vec n) = v := by
  ext j; simp [ofArr]

/-- Pivoted elimination with a step budget (`fuel >= length` suffices); counts pivots. -/
def gsRankF {n : ℕ} : ℕ → List (Vec n) → ℕ
  | 0, _ => 0
  | f + 1, L =>
    match selMax L with
    | none => 0
    | some (p, rest) => 1 + gsRankF f ((rest.map fun v => Array.ofFn (proj p v)).map (ofArr (n := n)))

/-- Exact numerical rank of a list of row vectors (the solver's `rank_qr` with tolerance 0). -/
def gsRank {n : ℕ} (L : List (Vec n)) : ℕ := gsRankF L.length L

/-- Rank of a row-major matrix given as a list of rows. -/
def rankOfRows (n : ℕ) (rows : List (List ℚ)) : ℕ :=
  gsRank (rows.map fun r => (fun j : Fin n => r.getD j 0 : Vec n))

section proofs
variable {n : ℕ}

theorem dot_self_eq_zero_iff (v : Vec n) : v ⬝ᵥ v = 0 ↔ v = 0 := dotProduct_self_eq_zero

theorem selMax_none {L : List (Vec n)} (h : selMax L = none) : ∀ v ∈ L, v = 0 := by
  intro v hv
  unfold selMax at h
  cases hm : L.argmax (fun v => v ⬝ᵥ v) with
  | none =>
    rw [List.argmax_eq_none] at hm; subst hm; simp at hv
  | some p =>
    rw [hm] at h
    simp only at h
    split_ifs at h with hp
    have := List.le_of_mem_argmax hv hm
    have hnn : 0 ≤ v ⬝ᵥ v := Finset.sum_nonneg fun i _ => mul_self_nonneg (v i)
    have h0 : v ⬝ᵥ v = 0 := le_antisymm (this.trans hp.le) hnn
    exact (dot_self_eq_zero_iff v).1 h0

theorem selMax_some {L : List (Vec n)} {p : Vec n} {rest : List (Vec n)}
    (h : selMax L = some (p, rest)) : L.Perm (p :: rest) ∧ p ⬝ᵥ p ≠ 0 := by
  unfold selMax at h
  cases hm : L.argmax (fun v => v ⬝ᵥ v) with
  | none => rw [hm] at h; simp at h
  | some q =>
    rw [hm] at h
    simp only at h
    split_ifs at h with hq
    obtain ⟨rfl, rfl⟩ := Prod.mk.inj (Option.some.inj h)
    exact ⟨List.perm_cons_erase (List.argmax_mem hm), hq⟩

/-- Spans of the lists as sets. -/
def spanL (L : List (Vec n)) : Submodule ℚ (Vec n) := Submodule.span ℚ {v | v ∈ L}

theorem spanL_perm {L L' : List (Vec n)} (h : L.Perm L') : spanL L = spanL L' := by
  unfold spanL; congr 1; ext v; simp [h.mem_iff]

theorem spanL_nil : spanL ([] : List (Vec n)) = ⊥ := by simp [spanL]

/-- Projection stays in the span of `{p, v}`, and `v` is recovered. -/
theorem spanL_cons_proj {p : Vec n} (L : List (Vec n)) (hp : p ⬝ᵥ p ≠ 0) :
    spanL (p :: L.map (proj p)) = spanL (p :: L) := by
  unfold spanL
  apply le_antisymm
  · apply Submodule.span_le.2
    intro v hv
    simp only [List.mem_cons, List.mem_map, Set.mem_setOf_eq] at hv
    rcases hv with rfl | ⟨w, hw, rfl⟩
    · exact Submodule.subset_span (by simp)
    · rw [proj_eq]
      refine Submodule.sub_mem _ (Submodule.subset_span (by simp [hw]))
        (Submodule.smul_mem _ _ (Submodule.subset_span (by simp)))
  · apply Submodule.span_le.2
    intro v hv
    simp only [List.mem_cons, Set.mem_setOf_eq] at hv
    rcases hv with rfl | hw
    · exact Submodule.subset_span (by simp)
    · have : v = proj p v + ((v ⬝ᵥ p) / (p ⬝ᵥ p)) • p := by rw [proj_eq]; simp
      rw [this]
      refine Submodule.add_mem _ (Submodule.subset_span ?_)
        (Submodule.smul_mem _ _ (Submodule.subset_span (by simp)))
      simp only [Set.mem_setOf_eq, List.mem_cons, List.mem_map]
      exact Or.inr ⟨v, hw, rfl⟩

/-- Everything in the projected list is orthogonal to the pivot. -/
theorem proj_orth {p : Vec n} (hp : p ⬝ᵥ p ≠ 0) (v : Vec n) : proj p v ⬝ᵥ p = 0 := by
  rw [proj_eq, sub_dotProduct, smul_dotProduct, smul_eq_mul, div_mul_cancel₀ _ hp, sub_self]

theorem finrank_cons {p : Vec n} (L : List (Vec n)) (hp : p ⬝ᵥ p ≠ 0)
    (horth : ∀ v ∈ L, v ⬝ᵥ p = 0) :
    Module.finrank ℚ (spanL (p :: L)) = Module.finrank ℚ (spanL L) + 1 := by
  have hset : {v | v ∈ p :: L} = insert p {v | v ∈ L} := by ext; simp
  have hnot : p ∉ spanL L := by
    intro hmem
    have horth' : ∀ u ∈ spanL L, u ⬝ᵥ p = 0 := by
      intro u hu
      refine Submodule.span_induction (p := fun u _ => u ⬝ᵥ p = 0) ?_ ?_ ?_ ?_ hu
      · intro x hx; exact horth x hx
      · simp
      · intro x y _ _ hx hy; rw [add_dotProduct, hx, hy, add_zero]
      · intro a x _ hx; rw [smul_dotProduct, hx, smul_zero]
    exact hp (horth' p hmem)
  unfold spanL
  rw [hset]
  exact finrank_span_insert_add_one hnot

/-- Core: with enough fuel, the elimination count is the dimension of the span. -/
theorem gsRankF_eq : ∀ (f : ℕ) (L : List (Vec n)), L.length ≤ f →
    gsRankF f L = Module.finrank ℚ (spanL L) := by
  intro f
  induction f with
  | zero =>
    intro L hL
    have : L = [] := List.length_eq_zero_iff.1 (Nat.le_zero.1 hL)
    subst this; rw [spanL_nil, finrank_bot]; simp [gsRankF]
  | succ f ih =>
    intro L hL
    cases hs : selMax L with
    | none =>
      have h0 := selMax_none hs
      have : spanL L = ⊥ := by
        unfold spanL
        rw [Submodule.span_eq_bot]
        intro v hv; exact h0 v hv
      rw [this, finrank_bot]; simp [gsRankF, hs]
    | some pr =>
      obtain ⟨p, rest⟩ := pr
      obtain ⟨hperm, hp⟩ := selMax_some hs
      have hlen : rest.length ≤ f := by
        have := hperm.length_eq; simp at this; omega
      have e1 : spanL L = spanL (p :: rest.map (proj p)) := by
        rw [spanL_perm hperm, spanL_cons_proj rest hp]
      rw [e1]
      simp only [gsRankF, hs, List.map_map]
      rw [show (ofArr ∘ fun v => Array.ofFn (proj p v)) = proj p from by
        funext v; exact ofArr_ofFn (proj p v)]
      rw [ih _ (by simpa using hlen), finrank_cons _ hp (by
        intro v hv; simp only [List.mem_map] at hv; obtain ⟨w, _, rfl⟩ := hv; exact proj_orth hp w)]
      omega

end proofs

/-- The elimination model computes `Matrix.rank` (tolerance 0, exact arithmetic). -/
theorem gsRank_eq_rank {m n : ℕ} (J : Matrix (Fin m) (Fin n) ℚ) :
    gsRank (List.ofFn J) = J.rank := by
  unfold gsRank
  rw [gsRankF_eq _ _ (by simp), Matrix.rank_eq_finrank_span_row]
  have hset : {v | v ∈ List.ofFn J} = Set.range J.row := by
    ext v; simp only [Set.mem_setOf_eq, List.mem_ofFn, Set.mem_range]; rfl
  unfold spanL
  rw [hset]

/-- DOF through the elimination model. -/
theorem dof_eq_gsRank {m n : ℕ} (J : Matrix (Fin m) (Fin n) ℚ) :
    n - gsRank (List.ofFn J) = n - J.rank := by rw [gsRank_eq_rank]

/-- The count does not depend on how rows are ordered. -/
theorem gsRank_perm {m n : ℕ} (J : Matrix (Fin m) (Fin n) ℚ) (σ : Equiv.Perm (Fin m))
    (τ : Equiv.Perm (Fin n)) : gsRank (List.ofFn (J.submatrix σ τ)) = gsRank (List.ofFn J) := by
  rw [gsRank_eq_rank, gsRank_eq_rank, rank_reindex]

end CraftSolver
