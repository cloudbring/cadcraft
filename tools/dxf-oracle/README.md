# DXF oracle (ezdxf)

Ticket cloudbring/artcraft-algorithms#196. Reads every DXF in a pinned corpus with ezdxf and with
cadcraft, dumps both as the same canonical JSON, compares them, and checks that cadcraft's DXF
output is clean to ezdxf. ezdxf shares no code with cadcraft, so agreement is independent evidence.

## Run

```sh
python3 -m venv .venv && .venv/bin/pip install ezdxf==1.4.5
DXF_ORACLE_PYTHON=.venv/bin/python \
DXF_ORACLE_CORPUS=/path/to/corpus/cad-ezdxf \   # craft-corpus manifests/cad-ezdxf.toml, or mozman/ezdxf at c498560bb491c357deda538360ed0c8fdb11f5a7
  tools/dxf-oracle/run.sh [--check | --write-floors]
```

`run.sh` checks the corpus against `corpus.sha256`, runs the Python dumper (always `python -I`),
builds and runs the Rust example `crates/io/examples/dxf_oracle`, and writes
`docs/verification/dxf-oracle.md`. `--check` exits 1 when a count in `floors.json` is not met.
`--write-floors` records the current counts (raise floors when a fix lands; never lower them to
pass).

## What is compared

Files: the 103 corpus files plus `examples/*.dxf` of this repo.

| Part | How |
|---|---|
| Parse | ezdxf strict `readfile` (with `recover` as a labelled fallback) and `cadcraft_io::read` (panics caught and reported). |
| Entities | Matched by (space, handle); leftovers paired in file order within (space, type). Type, layer, colour, linetype, lineweight and per-type geometry: points, radii, angles, bulges, spline control points / knots / weights / fit points, text, insert transforms, dimension type and points. Numbers: 1e-9 relative with an absolute floor of 1e-9; angles modulo 360. |
| Tables | Layers with their properties; blocks (base point, entity count); paper layouts; `$ACADVER`. |
| Round trip | `cadcraft_io::read` then `write` (DXF) then `read`; the second dump must equal the first. ezdxf `audit()` runs on every export and must report no errors; ezdxf's reading of the export is compared with its reading of the original. |

Conventions and the entity types with detailed geometry are in `dump_ezdxf.py` and
`crates/io/examples/dxf_oracle/dump.rs`. Types without detailed geometry are compared on type,
layer and properties; cadcraft's `Unknown` entities are listed per type.

## Files

- `dump_ezdxf.py` - the ezdxf side.
- `run.sh` - the pipeline. `floors.json` - the CI floors. `corpus.sha256` - the pinned corpus.
- `findings.md` - the hand-written findings, included in the report.
- The workflow `.github/workflows/dxf-oracle.yml` runs it path-filtered.
