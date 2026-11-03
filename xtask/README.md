# Workspace tasks

`version`, `stats`, and `ico` dispatch to `craft-xtask` using the policies in
`src/config.rs`. Version uses the shared defaults matching the eight identical
original implementations. Stats retains CAD's two-million-entry, no-symlink
policy without imposing the shared default's additional directory limit. Icons
keep caller-relative paths, input order and any square size from 1 through 256.

Layers uses the shared graph reader and checker with CAD's explicit policy.
The local formatter preserves the existing table and success output because
`craft_xtask::layers::run` only prints `layers: ok` and has no report-format
configuration. Invalid graphs use the shared checker's diagnostic wording.
External path dependencies are now correctly classified as external rather than
workspace members. The existing wasm package selection uses the same policy.

Parity remains local: the shared SURVEY identifies CAD's menu catalog/command
inventory as app-owned. The shared weighted-checklist report is not applicable,
and its command hook prints the full Markdown rather than CAD's one-line summary.
There is no compatible shared report portion to extract without changing output.
Assets and all other CAD-specific command orchestration remain local.

The original help text is retained, with shared task syntax appended. Shared
version also supports `tag`; malformed command arguments use shared usage errors.
Version refresh now captures Cargo's stderr instead of echoing successful update
logs. Stats retains its original permissive flag handling. Valid-task golden stdout is unchanged,
apart from stats naturally counting the smaller, changed source tree.

Run `cargo test -p xtask`, `cargo clippy -p xtask --all-targets -- -D warnings`,
`cargo fmt --check -p xtask`, and `cargo xtask layers`. Golden fixture provenance
and the intentionally controlled parity registry are in `tests/golden/README.md`.
