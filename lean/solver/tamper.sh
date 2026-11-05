#!/usr/bin/env bash
# Show that the gates fail when they should: each tamper must make check.sh (or the Rust test) fail.
set -uo pipefail
export PATH="$HOME/.elan/bin:$PATH"
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$here"
repo="$(cd "$here/../.." && pwd)"
csv=tests/vectors/dof.csv
cp "$csv" "$csv.orig"; cp CraftSolver/Dof.lean Dof.lean.orig; cp Audit.lean Audit.lean.orig
restore() { mv -f "$csv.orig" "$csv"; mv -f Dof.lean.orig CraftSolver/Dof.lean; mv -f Audit.lean.orig Audit.lean; }
trap restore EXIT
fail=0
expect_fail() { # name, command...
  local name="$1"; shift
  if "$@" > tamper.log 2>&1; then echo "TAMPER NOT DETECTED: $name"; fail=1; else echo "tamper detected: $name"; fi
}

# 1. Flip one rank in the CSV: regeneration must differ.
sed -i.bak 's/^rect_open,sketch,12,16,\(.*\),12,4,0$/rect_open,sketch,12,16,\1,11,5,1/' "$csv"; rm -f "$csv.bak"
expect_fail "csv rank changed (check.sh)" ./check.sh
expect_fail "csv rank changed (rust test)" env CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo/target-o}" cargo test -p cadcraft-constraints --lib lean_vectors
cp "$csv.orig" "$csv"

# 2. A sorry in a proved file.
printf '\ntheorem CraftSolver.tampered : (1 : Nat) = 2 := sorry\n' >> CraftSolver/Dof.lean
printf '#print axioms CraftSolver.tampered\n' >> Audit.lean
expect_fail "sorry added (check.sh)" ./check.sh
cp Dof.lean.orig CraftSolver/Dof.lean; cp Audit.lean.orig Audit.lean

# 3. A new axiom.
printf '\naxiom CraftSolver.badAx : False\ntheorem CraftSolver.tampered2 : (1 : Nat) = 2 := CraftSolver.badAx.elim\n' >> CraftSolver/Dof.lean
printf '#print axioms CraftSolver.tampered2\n' >> Audit.lean
expect_fail "axiom added (check.sh)" ./check.sh
cp Dof.lean.orig CraftSolver/Dof.lean; cp Audit.lean.orig Audit.lean

rm -f tamper.log
[ "$fail" -eq 0 ] && echo "tamper.sh OK: every tamper was caught" || { echo "tamper.sh FAILED"; exit 1; }
