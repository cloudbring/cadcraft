#!/usr/bin/env bash
# Run the ezdxf oracle over the corpus plus cadcraft's examples/*.dxf and write the report.
#
#   DXF_ORACLE_PYTHON  python with ezdxf installed (default: python3)
#   DXF_ORACLE_CORPUS  the pinned cad-ezdxf corpus (craft-corpus, manifests/cad-ezdxf.toml)
#   DXF_ORACLE_WORK    scratch directory (default: a new temp dir)
#   CARGO_TARGET_DIR   as usual
#
#   run.sh            write docs/verification/dxf-oracle.md
#   run.sh --check    also fail if a floor in tools/dxf-oracle/floors.json is violated
#   run.sh --write-floors   record the current counts as the floors
set -euo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
PY="${DXF_ORACLE_PYTHON:-python3}"
CORPUS="${DXF_ORACLE_CORPUS:?set DXF_ORACLE_CORPUS to the cad-ezdxf corpus directory}"
WORK="${DXF_ORACLE_WORK:-$(mktemp -d)}"
TARGET="${CARGO_TARGET_DIR:-$REPO/target}"
EXTRA=()
for a in "$@"; do EXTRA+=("$a"); done

# The corpus must be the pinned one (mozman/ezdxf at c498560bb491c357deda538360ed0c8fdb11f5a7).
if [ -z "${DXF_ORACLE_NO_VERIFY:-}" ]; then
  "$PY" -I - "$CORPUS" "$HERE/corpus.sha256" <<'PYEOF'
import hashlib, os, sys
root, listing = sys.argv[1], sys.argv[2]
bad = 0
for line in open(listing):
    digest, path = line.rstrip("\n").split("  ", 1)
    try:
        got = hashlib.sha256(open(os.path.join(root, path), "rb").read()).hexdigest()
    except OSError:
        got = None
    if got != digest:
        bad += 1
        print("corpus mismatch:", path, file=sys.stderr)
sys.exit(1 if bad else 0)
PYEOF
fi

rm -rf "$WORK/in" "$WORK/ez" "$WORK/rs" "$WORK/rt" "$WORK/audit"
mkdir -p "$WORK/in"
ln -s "$CORPUS" "$WORK/in/corpus"
ln -s "$REPO/examples" "$WORK/in/cadcraft-examples"
RELS=()
while IFS= read -r f; do RELS+=("$f"); done < <(cd "$WORK/in" && find -L corpus cadcraft-examples -type f \( -iname '*.dxf' \) | sed 's|^\./||' | LC_ALL=C sort)
echo "dxf-oracle: ${#RELS[@]} files; $("$PY" -I -c 'import ezdxf; print("ezdxf", ezdxf.__version__)')"

(cd "$REPO" && cargo build -q -p cadcraft-io --example dxf_oracle)
BIN="$TARGET/debug/examples/dxf_oracle"

"$PY" -I "$HERE/dump_ezdxf.py" "$WORK/ez" "$WORK/in" "${RELS[@]}" > /dev/null 2>&1
"$BIN" dump "$WORK/rs" "$WORK/in" "${RELS[@]}"
"$BIN" roundtrip "$WORK/rt" "$WORK/in" "${RELS[@]}"
EXPORTS=()
while IFS= read -r f; do EXPORTS+=("$f"); done < <(cd "$WORK/rt/dxf" && ls | LC_ALL=C sort)
if [ "${#EXPORTS[@]}" -gt 0 ]; then
  "$PY" -I "$HERE/dump_ezdxf.py" "$WORK/audit" "$WORK/rt/dxf" "${EXPORTS[@]}" > /dev/null 2>&1
else
  mkdir -p "$WORK/audit"
fi
"$BIN" report --ez "$WORK/ez" --rs "$WORK/rs" --rt "$WORK/rt" --audit "$WORK/audit" \
  --md "$REPO/docs/verification/dxf-oracle.md" --floors "$HERE/floors.json" --findings "$HERE/findings.md" "${EXTRA[@]}"
