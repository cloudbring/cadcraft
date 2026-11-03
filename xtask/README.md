# Workspace tasks

`version`, `stats`, and `ico` dispatch to `craft-xtask` using the policies in
`src/config.rs`. Version uses the shared defaults matching the eight identical
original implementations. Stats retains CAD's two-million-entry, no-symlink
policy without imposing the shared default's additional directory limit. Icons
keep caller-relative paths, input order and any square size from 1 through 256.

Layers uses `craft_xtask::layers::compute` / `check_graph` with CAD's policy and
formats the resulting `layers::Report` table locally (since `layers::render`
only prints `layers: ok` on success). The wasm package selection reads crates
from the same `layers::Report`.

Parity remains local: the shared `SURVEY.md` identifies CAD's menu catalog and
`cadcraft-cli commands` inventory as app-owned, not a 4-column `D/P/M` weighted
checklist. Assets and all other CAD-specific command orchestration remain local.

Run `cargo test -p xtask`, `cargo clippy -p xtask --all-targets -- -D warnings`,
`cargo fmt --check -p xtask`, and `cargo xtask layers`. Golden fixture provenance
and the intentionally controlled parity registry are in `tests/golden/README.md`.
