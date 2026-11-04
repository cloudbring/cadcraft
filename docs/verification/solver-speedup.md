# cadcraft constraint solver: re-measuring the 4,581x claim

Ticket: cloudbring/artcraft-algorithms#191 (V3.3). Claim C01 / C14 in `research/VERIFICATION.md`.
Measured 2026-11-04, 5:15 PM to 5:45 PM EST, on one machine (Apple M6, 12 cores, 32 GiB, macOS,
rustc 1.99.0, release profile as cadcraft's: opt-level 3, thin LTO).

Claim under test (#16): after the analytic Jacobian and RCM-ordered banded Cholesky, the solver
went from 2,886 ms to 0.63 ms (4,581x) on one synthetic 2,000-constraint sketch.

## Verdict

- **4,581x does not reproduce as a user-visible number.** It is not reproducible at all against
  cadcraft main's public API, because main refuses a 2,000-row component (`TooLarge`, cap 800
  rows). Against main with its two row gates lifted (the dense finite-difference path the "before"
  figure came from), the same 2,000-constraint point chain through `solve()` is **1,219x
  [95% CI 1,195, 1,244]**, not 4,581x.
- **The figure is a solver-core number.** The in-tree ignored test that produced it times only
  `Comp::run`. Re-run here it gives 2,605 to 2,618 ms before and 0.68 to 0.93 ms after (three
  runs, unpaired, no CI): about 2,800x to 3,900x. Public `solve()` on the same sketch costs 2.12 ms
  after, because model building, constraint interpretation and write-back are included. The 0.63 ms
  is not reached on this machine in either measurement.
- **Magnitude depends strongly on structure.** At 2,000 constraints, wall speedup is 1,219x (chain),
  1,224x (grid), 5,740x (random mix, one component), 2.46x (many small components). So "thousands
  of times" is true only for one large connected component; the claim cannot be quoted for
  sketches in general.
- **Real sketches: 1x to 12x, typically 2x to 6x.** 105 real sketch variants (see below) were timed.
  Geometric-mean speedup 1.75x (as imported) and 2.30x (perturbed). Below about 50 constraints the
  new solver is neutral to slightly slower (new/old up to 1.2x on some inputs). Above 1,500
  constraints real sketches reach 5x to 12x.
- **Solutions agree.** 129 of 132 inputs agree to within 1e-6 (median max difference 3e-11),
  2 fail identically in both, 1 differs by 1.4e-5 at equal residual (details below). Rank/DOF
  diagnostics are equal on all 132 inputs, but vacuous on the 10 inputs where both report
  `Unknown`.

Evidence level per VERIFICATION.md 1.2: the paired timings meet the speedup standard (10 ABBA
repetitions, bootstrap CI, quiet-machine guard passed, output checked, raw JSON with fingerprint
and both revs). Real inputs come from the pinned corpus. C01 can move to L3 for the *revised*
claim "orders of magnitude on one large component; 2x to 12x on real sketches", not for 4,581x.

## What was compared

| Label | What | Rev |
|---|---|---|
| old | cadcraft main, public API (`cadcraft_constraints::solve`) | `f6d84e28f31ff98224a2b63b04e5153f4899ae2b` (merge-base of `algo/16-analytic-jacobian` with `origin/main`) |
| oldx | old with `lm::MAX_ROWS` 800 to 20,000 and the `rows^2 * free > 4e8` gate removed, nothing else (`tools/solver-bench/old-uncapped.patch`) | same, patched |
| new | the #16 branch, path dependency | `8574b50aeb852596e4e2e98204191fc23b210b9a` |

Why oldx exists: main's `Comp::run` returns `TooLarge` above 800 rows, so main cannot solve a
2,000-constraint component at all. The "2,886 ms before" in #16 was the in-tree
`solve_dense_legacy` (numeric Jacobian, dense elimination, no cap), which is main's algorithm with
the cap lifted. oldx reproduces that path through the public API. Every input is timed against
oldx; inputs main can also solve (at most 800 rows per component) are timed against main as well.
The two comparisons agree to within noise where both exist (see `data/solver-tables.md`).

The timed operation is `solve(&mut drawing.clone(), &SolveOptions::default())`, identical on both
sides. `Drawing::clone` shares entities by `Arc` and costs 3 us at 50 constraints, 46 us at 2,000,
116 us at 5,000 (column `clone us` in the tables): about 2% to 3% of the new solve time. Work shorter
than 100 us is batched inside the closure (same batch both sides) so the clock resolves it.

## Method

Harness: `tools/solver-bench/` (own `[workspace]`, own `Cargo.lock`; `cargo run --release` with
`run --arm main|uncapped`; `analyze.py` writes the tables). craft-bench from
cloudbring/craft-libs rev `2c2c69b4963ff5afb612152143ecd0ccd3b925cf`.

1. Sketches are built once as neutral JSON (entities and constraints through serde) and rebuilt
   on each side; handle lists are asserted equal.
2. **Agreement is checked before any timing**: both sides solve each input once; compared are the
   result kind, every entity parameter (max absolute difference), the max constraint residual
   before and after, `all_satisfied`, and `dof()` (dof, status, redundant count, constraint count)
   before and after. Only inputs where both succeed and the max parameter difference is at most
   1e-6 are timed.
3. craft-bench paired runs: 10 timed runs per side per input (ABBA blocks), warm-up 2 (1 for
   inputs whose old solve takes over a second), 2,000 bootstrap resamples, seed 0xC0FFEE, wall and
   process CPU time. Speedup below is old median / new median, CI is `[1/ci_hi, 1/ci_lo]` of the
   craft-bench new/old ratio.
4. Quiet-machine guard: the machine was not idle (browser and editor running), load average 2.9 to
   4.8 on 12 cores, below the guard limit of 6.0 (0.5 per core). No report was marked contended and
   all are `publishable: true`. Load was read from the guard in every JSON file.

### Inputs

- **Synthetic, sizes 50, 200, 500, 1,000, 2,000, 5,000 constraints**, four shapes:
  `chain` (verbatim from `crates/constraints/src/bench.rs`: points joined by horizontal and
  vertical Distance, so `chain-2000` is the original benchmark sketch), `grid` (k x k lattice of
  points, Distance on every edge), `mix` (joined line path in groups of six lines plus a circle:
  Coincident, Horizontal, Vertical, Parallel, Perpendicular, aligned Distance, Equal, Radius; one
  connected component; geometry satisfies all constraints, then every coordinate is perturbed by
  up to 0.05), `clustered` (same mix cut into components of about 24 constraints).
- **Real:** `examples/apartment.dxf` plus the pinned `cad-ezdxf` corpus (103 files fetched with
  `craft-corpus`; manifest `crates/corpus/manifests/cad-ezdxf.toml`). Each DXF is imported with
  `cadcraft_io::read_dxf`; model-space Line, Arc, Circle, Point and LwPolyline entities (first
  1,200 per file) are auto-constrained with cadcraft's own `infer` + `add` in chunks of 40
  entities (1.5 s budget per chunk), as `autoconstrain` does. 54 of 104 files yield at least two
  constraints (others have no constrainable entities; `bin_dxf_r12.dxf` fails to import).
  Each yields two variants: `@exact` (geometry as imported; the solver starts converged) and
  `@jitter` (every coordinate perturbed by 1% of the median entity size; the solver has work to
  do). 108 real inputs, 105 timeable. The sketches are derived from corpus files and cached in
  `tools/solver-bench/.cache/`, which is gitignored. No corpus file is committed.
- cadcraft ships no sketch fixtures beyond `examples/apartment.dxf` (its solver tests build
  drawings in code, which the synthetic chain reuses).

## Speedup versus size (synthetic; old = oldx, wall, 95% CI)

| constraints | chain | grid | mix (1 component) | clustered (~24 per component) |
|---:|---|---|---|---|
| 50 | 5.20x [4.94, 5.51] | 6.00x [5.57, 6.24] | 10.6x [9.36, 11.2] | 2.57x [2.32, 2.67] |
| 200 | 29.9x [27.5, 31.9] | 32.7x [31.0, 34.0] | 92.4x [87.4, 99.4] | 2.61x [2.54, 2.65] |
| 500 | 128x [126, 135] | 134x [121, 140] | 475x [466, 503] | 2.62x [2.56, 2.66] |
| 1,000 | 407x [399, 428] | 419x [404, 432] | 1,682x [1,665, 1,709] | 2.58x [2.53, 2.62] |
| **2,000** | **1,219x [1,195, 1,244]** | 1,224x [1,202, 1,255] | 5,740x [5,615, 5,808] | 2.46x [2.42, 2.51] |
| 5,000 | 3,562x [3,553, 3,636] | 3,328x [3,271, 3,367] | 3,595x [3,523, 3,656] | 2.48x [2.47, 2.54] |

Absolute medians at 2,000 constraints (ms, old / new): chain 2,586.8 / 2.123, grid 2,706.4 / 2.211,
mix 21,499 / 3.746, clustered 13.55 / 5.50. CPU-time speedups match wall within about 1% everywhere
(single thread). All 24 rows, with CPU time, are in `data/solver-tables.md`.

Reading the curve:

- Old cost grows roughly cubically in rows (dense elimination), new roughly linearly (banded
  Cholesky), so the speedup of one connected component grows about quadratically: 5x at 50, 30x at
  200, 130x at 500, 410x at 1,000, 1,220x at 2,000, 3,560x at 5,000.
- The mix at 2,000 is an outlier on the old side (21.5 s against 23.9 s at 5,000): the old solver
  needed many more Gauss-Newton iterations on that perturbed instance, so 5,740x measures
  convergence behaviour as well as factor cost. It is one instance, not a trend.
- With components of about 24 rows the old dense path is already cheap, and the new solver gains
  only 2.5x at every size. Drawings made of many small, independent constraint groups would sit here.

### Same comparison against cadcraft main itself (old = main)

Where main can solve the input (at most 800 rows per component) the speedups agree with the oldx
arm: chain 4.92x / 30.6x / 130x at 50 / 200 / 500, grid 6.05x / 32.9x / 138x, mix 10.9x / 94.3x at
50 / 200, clustered 2.60x to 2.62x at 50 to 500, and 2.55x, 2.48x, 2.48x at 1,000 / 2,000 / 5,000
(clustered components stay under the cap). **Not timed, because main returns `TooLarge` while new
returns a converged answer:** mix-500 (its component needs more than 800 rows), and chain, grid
and mix at 1,000, 2,000 and 5,000. That is 10 of 24 synthetic inputs. For these the correct
statement is "main refuses, new solves in 0.4 to 6.6 ms", a capability change and not a speedup.
Full table: `data/solver-tables.md`, section `arm main`.

## Real sketches (105 timed; old = oldx, wall)

| constraints | `@exact`: n, geo-mean speedup, min..max | `@jitter`: n, geo-mean speedup, min..max |
|---|---|---|
| 0 to 49 | 24, 1.06x, 0.88..4.44x | 24, 1.42x, 0.87..5.94x |
| 50 to 199 | 16, 1.80x, 0.84..7.97x | 16, 2.48x, 0.85..7.65x |
| 200 to 499 | 7, 3.00x, 0.93..6.12x | 7, 4.94x, 2.82..12.27x |
| 500 to 1,999 | 4, 6.16x, 5.13..8.75x | 4, 5.66x, 4.72..8.96x |
| 2,000 and up | 2, 7.35x, 5.41..9.98x | 1, 9.76x, 9.76..9.76x |

All inputs: `@exact` geometric mean 1.75x (23 faster, 23 slower, 7 inconclusive by CI against
1.0), `@jitter` 2.30x (38 faster, 8 slower, 6 inconclusive). The largest, with 95% CIs:

| input | constraints | old ms | new ms | speedup |
|---|---:|---:|---:|---|
| edgeminer/4_world.dxf @jitter | 2,437 | 205.5 | 21.06 | 9.76x [8.92, 9.81] |
| edgeminer/4_world.dxf @exact | 2,437 | 205.6 | 20.61 | 9.98x [9.88, 10.1] |
| tests recover01.dxf @exact | 2,634 | 102.2 | 18.88 | 5.41x [5.38, 5.47] |
| edgeminer/3_us_main.dxf @jitter | 1,587 | 21.9 | 2.447 | 8.96x [8.70, 9.06] |
| dxf/text_fonts.dxf @jitter | 1,512 | 11.3 | 2.392 | 4.72x [4.63, 4.79] |
| wipeout_door.dxf @jitter | 332 | 8.91 | 0.726 | 12.3x [11.6, 12.5] |

Every per-input row (105, both arms) is in `data/solver-tables.md` and, with raw per-run times, in
`data/solver-{main,uncapped}-*.json`.

The slower cases are small sketches solved in about 0.12 to 0.25 ms (after batching), such as
`columns_R2007.dxf` (120 constraints, new/old 1.19x, CI [1.13, 1.26]); the new path has fixed
overhead (cluster decomposition, Jacobian assembly) that a tiny dense solve does not repay. The
effect is real but at most about 20% of a sub-millisecond solve.

Caveat on realism: the constraints are inferred from geometry, not authored. Auto-constraining in
chunks of 40 entities means real inputs are made of small components, which is the regime where
the new solver gains least (compare the `clustered` column). A hand-constrained sketch with one
large rigid component would sit on the `chain`/`grid` curve. No such sketch exists in the corpus.

## Agreement (checked before timing)

oldx vs new, 132 inputs (24 synthetic, 108 real):

| verdict | count |
|---|---:|
| agree (both converge, max parameter difference at most 1e-6) | 129 |
| both fail the same way | 2 |
| differ | 1 |

- Max parameter difference over the 130 comparable inputs: median 2.96e-11, 90th percentile
  2.56e-9, maximum 1.39e-5. Synthetic inputs: all within 1.2e-9 (the 7.1e-15 in #16 is the
  sparse-vs-dense difference on one fixture, not this comparison).
- Max constraint residual after solving: old 2.42e-5, new 2.42e-5 (both worst on the same input,
  `houses_of_parliament_georeferenced.dxf`, where the tolerance is scaled to coordinate size).
- **DOF/rank**: the `dof()` report (dof, status, redundant count, constraint count) is equal on all
  132 inputs before and after solving. For 10 inputs (the large ones) both sides return
  `dof=None, status=Unknown` because the rank-work budget in `dof.rs` is exceeded, so equality there
  carries no information; 120 inputs report a definite dof with status `Under`, 2 report `Over`.
- Uninterpretable constraints (skipped): 5,020 on each side, identical (the inferred sets contain
  kinds the solver cannot yet interpret on some entities); no synthetic input skips any.
- `examples/edgeminer/5_us_states.dxf` (both variants, 2,285 constraints): both sides return the
  same `Conflict` with the same residual 4.97e-2. Not timed; an inferred set that is not
  consistent.
- **The one difference:** `recover01.dxf@jitter` (2,634 constraints, 824 free DOF). Max parameter
  difference 1.39e-5; max residual 9.36e-7 on both sides to five digits; the same dof. The system
  is under-determined and both reach the tolerance on different points of the solution manifold
  (new starts rigid clusters from a warm start, old from the given geometry; per the code comments in `lib.rs`), so the difference
  is most likely a drift along the null space inside the solver's tolerance (not investigated
  further). It is reported rather than
  waved through; it fails the 1e-6 gate, and its timing was excluded. `recover01@exact` agrees
  (3.6e-9).
- Against main (not oldx), the 10 one-sided failures are all `TooLarge` from main, listed above.
  Remember that #16 also states main falsely reported redundancy/Conflict on one fixture; this run
  did not search for that.

## Reproducing the original number

`cargo test -p cadcraft-constraints --release --lib synthetic_chain_before_after_ms -- --ignored
--nocapture` on this branch, three runs (first run saved as `data/intree-bench-run1.log`):

| rows | dense-fd ms | analytic-sparse ms | ratio |
|---:|---|---|---|
| 200 | 4.49 to 4.81 | 0.084 to 0.119 | 38x to 57x (claim 44x) |
| 800 | 187.0 to 190.5 | 0.289 to 0.329 | 570x to 660x (claim 650x) |
| 2,000 | 2,605 to 2,618 | 0.678 to 0.925 | 2,800x to 3,860x (claim 4,580x) |

These are unpaired single-shot numbers without a CI; the new side varies 36% between runs, which is
why the ratio does. 4,581x needs a 0.63 ms new side; the best of three here is 0.678 ms. The old
side (2,605 ms) is 10% below the claim's 2,886 ms.

## Limits of this measurement

- One machine, one session, not idle (load 2.9 to 4.8 of a 6.0 limit). Ratios cancel machine
  speed, absolute medians do not.
- oldx is main with two gates lifted, built from patched source (`old-uncapped.patch`, recreated
  by `tools/solver-bench/setup-old-uncapped.sh`). It is not a released build.
- Real sketches are corpus DXFs with machine-inferred constraints, truncated to 1,200 entities and
  built from 40-entity chunks; no dimensional constraints, no user-authored sketches.
- `mix-2000` old iteration count is anomalous (see above); one instance.
- The new side's `build`/`write_back` cost is included by design (it is what a caller pays), so
  speedups here are lower than the solver-core ratio the claim quoted.

## Files

- `tools/solver-bench/` harness (`Cargo.toml`, `Cargo.lock`, `src/`, `analyze.py`,
  `setup-old-uncapped.sh`, `old-uncapped.patch`).
- `docs/verification/data/solver-{uncapped,main}-{real,synthetic-fast,synthetic-slow}.json`: raw
  craft-bench reports (per-run times, fingerprint, revs, guard).
- `docs/verification/data/agreement-{uncapped,main}.json`: per-input agreement rows.
- `docs/verification/data/solver-tables.md`: all generated tables.
- `docs/verification/data/intree-bench-run1.log`: the original in-tree benchmark, re-run.

Reproduce: `tools/solver-bench/setup-old-uncapped.sh`, fetch the corpus with
`craft-corpus fetch crates/corpus/manifests/cad-ezdxf.toml <dir>` (craft-libs rev 2c2c69b), then
`cd tools/solver-bench && CARGO_TARGET_DIR=<tmp> cargo run --release -- run --arm uncapped
--corpus <dir> --old-rev f6d84e2... --new-rev 8574b50...` (and `--arm main`).
