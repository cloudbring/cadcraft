## Findings

Every row below was traced to the file and handle given in the tables above (or in the per-file
list). "Fixed" rows have a red-then-green test in this change.

### Fixed in this change

| # | Finding | Root cause | Fix and test |
|---|---|---|---|
| F1 | `integration_tests/data/bin_dxf_r12.dxf` (Release 12 binary DXF) failed to open: `no sections found`. | `cadcraft_dxf::parse_binary` read every group code as two bytes (R13 and later). Release 12 writes one byte (255 introduces a two-byte code), so the first tag read as code 21248. | `crates/dxf/src/lib.rs`: a file whose first byte is `00` followed by a letter has one-byte codes. Test `crates/dxf/tests/binary_r12.rs` (failed with `code: 21248`, passes). |
| F2 | A two-line angular dimension whose lines meet at an end point they share (`examples_dxf/uncommon.dxf`, handle 3AE) was measured as 2 pi (6.2832) when saved. ezdxf and the file say 2.2020 rad (126.2 degrees). | `draw_angular` took the DXF points 14 and 15 as the far ends of the two lines; when a line starts at the vertex that point is the vertex and the leg has no direction. It also only chose between the angle and its complement, but the arc location (group 16) selects one of the four angles two lines make. | `crates/render/src/dim/variants.rs`: far ends are used, and the sector the arc location lies in is measured. Test `crates/render/tests/it/dim_angular_shared_endpoint.rs` (measured 6.283 before, passes). |

### Open (not fixed here)

| # | Finding | Evidence | Why not fixed |
|---|---|---|---|
| O1 | Saving rewrites the text position (group 11) and measurement (group 42) of dimensions, so the first save changes what a re-import sees: 5 files do not round-trip to an equal dump (`apartment.dxf`, `dimension_in_block.dxf`, `dimension_in_nested_blocks.dxf`, `uncommon.dxf`, `duplicate_handles.dxf`), all through `DIMENSION` fields only. The text point moves (apartment.dxf#132: x -590 to -576; dimension_in_block.dxf#9D: 5.85 to 6.07). | Round-trip section; "Where the export differs" table. | The new position comes from cadcraft's own text metrics; whether to keep the file's value for dimensions that are not user-positioned is a product decision. The second save is stable. |
| O2 | A file without `$ACADVER` is taken as AC1032 (R2018) and saved as that; ezdxf takes it as AC1009. | `Minimal_DXF_AC1009.dxf`, `R12_with_trash_beyond_EOF.dxf`. | Needs a rule for what a missing version means. |
| O3 | Layer `Defpoints` without group 290 is plottable after import; ezdxf (and AutoCAD) never plot it. | `layer.plot` in `duplicate_handles.dxf`, `r12_blocks_with_no_names.dxf`. cadcraft's own new drawings create it non-plotting. | Small, but changes what a plot shows for old files; wants its own ticket and plot test. |
| O4 | Dropped on import: block records without a BLOCK (`Leica_Disto_S910.dxf`: `RH`, `SP`, `single_point`; `r2000_blocks_with_no_names.dxf`), and a polyface mesh with vertices but no face records (`uncommon.dxf`#4D7). Kept oddly: a block named `""`, and R12 `$MODEL_SPACE` / `$PAPER_SPACE` as ordinary blocks. | Per-file list. | Harmless to the drawing; the odd names should be checked against the writer. |
| O5 | Types kept as `Unknown` (passed through unchanged, not drawn or edited): `3DSOLID` 12, `MESH` 1, `ARC_DIMENSION` 1, `LARGE_RADIAL_DIMENSION` 2. | Unsupported table. | Feature work. They survive the round trip (the export has the same counts). |
| O6 | 126 TEXT and MTEXT entities with extrusion (0, 0, -1) are compared on type, layer and properties only. cadcraft re-expresses them in world coordinates (justification and rotation change, e.g. right-justified becomes left); the oracle does not yet derive that form independently. | `text_negative_extrusion.dxf` (92), `POLI-ALL210_12.DXF` (30), `mtext_complex_inline_codes.dxf` (4). | Needs an independent definition of the mirrored text's ink position (a rendered comparison), not a field compare. |

### Expected differences (cadcraft is right, or the file is invalid)

| Difference | Why |
|---|---|
| `MTEXT.text` with `Ã‚Â²` in ezdxf (15, `apartment.dxf`) | The file says `$DWGCODEPAGE ANSI_1252` but holds UTF-8; ezdxf obeys the header, cadcraft recognises UTF-8. |
| `MTEXT.width` (15, column files) | cadcraft keeps the column width (embedded object); group 41 is the total width. See `dxf_read/text.rs`. |
| Extra `MTEXT`/`LWPOLYLINE` only in cadcraft (column files, framed text) | cadcraft splits columns and frames into separate entities. |
| `ACAD_TABLE.layer` (`apartment.dxf`#131) | The file says `A-ANNO`; ezdxf reports `0` (it ignores the entity subclass that carries the layer). |
| `CIRCLE.radius` -1 and `ELLIPSE.ratio` above 1 | Invalid values, normalised by cadcraft (`abs`, clamp to 1). |
| `duplicate_handles.dxf` | Handles are not unique, so entities cannot be matched by handle. |
| Files without handles (R12 and older) | Entities are paired in file order within space and type; the table above counts them. |
| Layers `HATCH`, `DYNAMIC`, `STATIC`, `INSERT_POINTS`, `Layer_is_a_required_tag` only in cadcraft | Used by entities but absent from the LAYER table; cadcraft creates them. |
| `$ACADVER` AC1006/AC1012/AC1014 against AC1009/AC1015 | ezdxf reports the version it upgrades to; the oracle reads `loaded_dxfversion`. |
| Dimension `*D`, table `*T` and standard arrowhead blocks | Generated by the program that writes the file; cadcraft rebuilds them on save. Their entities are left out of the statistics (counts above). |
