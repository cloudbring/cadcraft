# CADCraft xtask compatibility snapshots

Captured before migration from `f6d84e28f31ff98224a2b63b04e5153f4899ae2b`
(also local `main`) on 2026-11-03. Each fixture is below 200 KB.
Only first-party source and synthetic data were used; no reference application
files, assets, or implementation were inspected.

- `layers.stdout`, `stats.stdout`, and `version.stdout`: original task output on
  the current tree. `metadata.json` retains the package/dependency fields consumed
  by the layer checker. IDs and paths are normalized for portability.
- `stats-counts.json`: original per-package counts replayed as synthetic Rust
  files so this formatting/traversal regression does not depend on future app
  source changes. Separate tests cover comments, exclusions, symlinks and budgets.
- `version-inputs.json`, `version-before.lock`: original workspace manifests,
  target paths and lockfile. The original version task ran `set 1.2.3-rc.1` on a
  temporary copy with empty target source placeholders and a real Cargo lockfile
  refresh. `version-after.toml`, `version-after.lock`, `version-set.stdout` are
  its captured results. No application crate was compiled. Tests replay the
  real refresh in a child test process, stripping only libtest's fixed startup
  header before comparing stdout; dependencies must be cached or reachable by Cargo.
- `icon-*.bin`: synthetic PNG headers at 16/32/48/128/256 pixels, in that order;
  `ico.bin` and `ico.stdout` are the original packer's bytes and output. Binary
  extensions avoid treating these intentionally incomplete test inputs as artwork.
- `menu-catalog.txt`: the existing first-party menu-name inventory.
  `commands.json`: controlled command registry with a hit, misses and an empty
  menu. Original parity ran with a Cargo stand-in returning these bytes, capturing
  `parity.md` and `parity.stdout`. Tests inject the captured date, not today's date.
  This validates the inventory/report pipeline, not the live CLI registry; building
  cadcraft-cli was explicitly outside this ticket's verification scope.
