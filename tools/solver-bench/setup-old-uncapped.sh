#!/bin/sh
# Recreate .old-uncapped: the merge-base source of geom, color, doc and constraints, with the
# workspace inheritance inlined (so it cannot be confused with the #16 crates), the packages
# renamed *-oldx, and the two row gates lifted (old-uncapped.patch, source only).
set -eu
cd "$(dirname "$0")"
MB=f6d84e28f31ff98224a2b63b04e5153f4899ae2b
rm -rf .old-uncapped && mkdir -p .old-uncapped/crates
for c in geom color doc constraints; do
  git -C ../.. archive "$MB" "crates/$c" | tar -x -C .old-uncapped
done
patch -p1 -d .old-uncapped < old-uncapped.patch
python3 - <<'PY'
import re
deps = {
 "serde": 'serde = { version = "1", features = ["derive", "rc"] }',
 "serde_json": 'serde_json = { version = "1", features = ["float_roundtrip"] }',
 "thiserror": 'thiserror = "2"',
}
for c in ["geom", "color", "doc", "constraints"]:
    p = f".old-uncapped/crates/{c}/Cargo.toml"
    s = open(p).read()
    s = s.replace('name = "cadcraft-%s"' % c, 'name = "cadcraft-%s-oldx"' % c)
    s = s.replace("version.workspace = true", 'version = "0.17.0-oldx"').replace("edition.workspace = true", 'edition = "2024"').replace("license.workspace = true", 'license = "MIT OR Apache-2.0"')
    s = s.replace("[lints]\nworkspace = true\n\n", "")
    def sub(m):
        k = m.group(1)
        if k.startswith("cadcraft-"):
            n = k.split("-", 1)[1]
            return f'{k} = {{ package = "{k}-oldx", path = "../{n}" }}'
        return deps[k]
    s = re.sub(r'^([\w-]+) = \{ workspace = true \}$', sub, s, flags=re.M)
    open(p, "w").write(s)
open(".old-uncapped/Cargo.toml", "w").write('[workspace]\nresolver = "3"\nmembers = ["crates/geom", "crates/color", "crates/doc", "crates/constraints"]\n')
PY
