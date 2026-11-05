//! The report: per-file and total statistics, the Markdown document and the CI floors.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::compare::{Fields, FileCmp, TypeStat, compare_file};

pub struct Opts {
    pub ez: PathBuf,
    pub rs: PathBuf,
    pub rt: PathBuf,
    pub audit: PathBuf,
    pub md: PathBuf,
    pub floors: Option<PathBuf>,
    pub check: bool,
    pub write_floors: bool,
    pub findings: Option<PathBuf>,
}

pub fn fields_json(f: &Fields) -> Value {
    Value::Object(f.0.iter().map(|(k, (n, ex))| (k.clone(), json!({"count": n, "example": ex}))).collect())
}

fn load(dir: &Path, name: &str) -> Value {
    std::fs::read_to_string(dir.join(name)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null)
}

/// What the round trip and the ezdxf audit of the export say about one file.
#[derive(Default, Clone)]
struct Trip {
    ran: bool,
    stage_failed: String,
    error: String,
    equal: bool,
    audit_errors: usize,
    audit_messages: Vec<String>,
    audit_ran: bool,
    /// ezdxf's view of the export against its view of the original.
    writer: FileCmp,
}

struct Row {
    cmp: FileCmp,
    trip: Trip,
}

fn trip_of(name: &str, ez: &Value, o: &Opts) -> Trip {
    let rt = load(&o.rt, name);
    let mut t = Trip::default();
    if rt.is_null() {
        return t;
    }
    t.ran = true;
    if rt["ok"].as_bool() != Some(true) {
        t.stage_failed = rt["stage"].as_str().unwrap_or("?").to_string();
        t.error = rt["error"].as_str().unwrap_or("").to_string();
        return t;
    }
    t.equal = rt["equal"].as_bool().unwrap_or(false);
    let exported = load(&o.audit, name);
    if exported.is_null() {
        return t;
    }
    t.audit_ran = exported["audit"].is_object();
    let errs = exported["audit"]["errors"].as_array().cloned().unwrap_or_default();
    t.audit_errors = errs.len();
    t.audit_messages = errs.iter().take(3).map(|e| e["message"].as_str().unwrap_or("").to_string()).collect();
    if exported["ok"].as_bool() != Some(true) {
        t.audit_errors += 1;
        t.audit_messages.push(format!("ezdxf cannot read the export: {}", exported["error"].as_str().unwrap_or("")));
    }
    if ez["ok"].as_bool() == Some(true) && exported.get("entities").is_some() {
        t.writer = compare_file(name, ez, &exported);
    }
    t
}

fn pct(a: usize, b: usize) -> String {
    if b == 0 { "n/a".into() } else { format!("{:.2}%", 100.0 * a as f64 / b as f64) }
}

#[derive(Default)]
struct Totals {
    files: usize,
    ez_ok: usize,
    ez_recovered: usize,
    rs_ok: usize,
    both: usize,
    entities: usize,
    matched: usize,
    agree: usize,
    by_type: BTreeMap<String, TypeStat>,
    fields: Fields,
    layer_fields: Fields,
    unsupported: BTreeMap<String, (usize, usize)>,
    version_equal: usize,
    layer_note_files: usize,
    block_note_files: usize,
    missing: usize,
    extra: usize,
    rt_ran: usize,
    rt_equal: usize,
    audit_files: usize,
    audit_clean: usize,
    audit_errors: usize,
    audit_errors_orig: usize,
    writer: BTreeMap<String, TypeStat>,
    writer_fields: Fields,
    by_order: usize,
    ocs: usize,
    generated: (usize, usize),
}

fn totals(rows: &[Row]) -> Totals {
    let mut t = Totals { files: rows.len(), ..Default::default() };
    for r in rows {
        let c = &r.cmp;
        t.ez_ok += usize::from(c.ez_ok);
        t.ez_recovered += usize::from(c.ez_recovered);
        t.rs_ok += usize::from(c.rs_ok);
        for (k, n) in &c.unsupported {
            let s = t.unsupported.entry(k.clone()).or_default();
            s.0 += n;
            s.1 += 1;
        }
        if r.trip.ran {
            t.rt_ran += 1;
            t.rt_equal += usize::from(r.trip.equal);
        }
        if r.trip.audit_ran {
            t.audit_files += 1;
            t.audit_clean += usize::from(r.trip.audit_errors == 0);
            t.audit_errors += r.trip.audit_errors;
        }
        for (k, s) in &r.trip.writer.by_type {
            let w = t.writer.entry(k.clone()).or_default();
            w.ez += s.ez;
            w.rs += s.rs;
            w.matched += s.matched;
            w.agree += s.agree;
        }
        t.writer_fields.merge(&r.trip.writer.entity_fields);
        if !c.compared() {
            continue;
        }
        t.audit_errors_orig += c.audit_errors;
        t.by_order += c.by_order;
        t.ocs += c.ocs_skipped;
        t.generated.0 += c.generated.0;
        t.generated.1 += c.generated.1;
        t.both += 1;
        t.version_equal += usize::from(c.version.0.eq_ignore_ascii_case(&c.version.1));
        t.layer_note_files += usize::from(!c.layer_notes.is_empty());
        t.block_note_files += usize::from(!c.block_notes.is_empty());
        t.missing += c.missing_in_rs.len();
        t.extra += c.extra_in_rs.len();
        t.entities += c.ez_entities();
        t.matched += c.matched();
        t.agree += c.agree();
        for (k, s) in &c.by_type {
            let a = t.by_type.entry(k.clone()).or_default();
            a.ez += s.ez;
            a.rs += s.rs;
            a.matched += s.matched;
            a.agree += s.agree;
        }
        t.fields.merge(&c.entity_fields);
        t.layer_fields.merge(&c.layer_fields);
    }
    t
}

fn type_table(md: &mut String, by: &BTreeMap<String, TypeStat>, ref_name: &str, got_name: &str) {
    let _ = writeln!(md, "| Type | {ref_name} | {got_name} | Matched by handle | Agree | Agreement |\n|---|---:|---:|---:|---:|---:|");
    for (k, s) in by {
        let _ = writeln!(md, "| {k} | {} | {} | {} | {} | {} |", s.ez, s.rs, s.matched, s.agree, pct(s.agree, s.ez));
    }
}

fn fields_table(md: &mut String, f: &Fields, limit: usize) {
    let mut v: Vec<_> = f.0.iter().collect();
    v.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(a.0.cmp(b.0)));
    let _ = writeln!(md, "| Field | Disagreements | Example (reference vs cadcraft) |\n|---|---:|---|");
    for (k, (n, ex)) in v.into_iter().take(limit) {
        let _ = writeln!(md, "| `{k}` | {n} | `{}` |", ex.replace('|', "\\|"));
    }
}

fn per_file(md: &mut String, rows: &[Row]) {
    let _ = writeln!(
        md,
        "| File | ezdxf parse | cadcraft parse | Entities (ezdxf) | Matched | Agree | Agreement | Missing in cadcraft | Extra in cadcraft | ezdxf audit (original) | Round trip equal | ezdxf audit (export) |\n|---|---|---|---:|---:|---:|---:|---:|---:|---:|---|---:|"
    );
    for r in rows {
        let c = &r.cmp;
        let ez = if c.ez_ok {
            "ok"
        } else if c.ez_recovered {
            "failed (recovered)"
        } else {
            "failed"
        };
        let rs = if c.rs_ok { "ok".to_string() } else { format!("FAILED: {}", c.rs_error.chars().take(50).collect::<String>().replace('|', "/")) };
        let rt = if !r.trip.ran {
            "-".to_string()
        } else if !r.trip.stage_failed.is_empty() {
            format!("failed at {}", r.trip.stage_failed)
        } else if r.trip.equal {
            "yes".into()
        } else {
            "NO".into()
        };
        let au = if r.trip.audit_ran { r.trip.audit_errors.to_string() } else { "-".into() };
        let _ = writeln!(
            md,
            "| `{}` | {ez} | {rs} | {} | {} | {} | {} | {} | {} | {} | {rt} | {au} |",
            c.file,
            c.ez_entities(),
            c.matched(),
            c.agree(),
            pct(c.agree(), c.ez_entities()),
            c.missing_in_rs.len(),
            c.extra_in_rs.len(),
            c.audit_errors
        );
    }
}

/// What differs in each file, a few examples each, so a reader can start from the file.
fn details(md: &mut String, rows: &[Row]) {
    for r in rows {
        let c = &r.cmp;
        if !c.compared() {
            continue;
        }
        let ver = !c.version.0.eq_ignore_ascii_case(&c.version.1);
        if c.missing_in_rs.is_empty() && c.extra_in_rs.is_empty() && c.layer_notes.is_empty() && c.block_notes.is_empty() && !ver {
            continue;
        }
        let _ = writeln!(md, "- `{}`", c.file);
        if ver {
            let _ = writeln!(md, "  - $ACADVER: ezdxf {:?}, cadcraft {:?}", c.version.0, c.version.1);
        }
        let mut list = |label: &str, v: &[String]| {
            if !v.is_empty() {
                let _ = writeln!(md, "  - {label} ({}): {}", v.len(), v.iter().take(4).map(|s| format!("`{s}`")).collect::<Vec<_>>().join(", "));
            }
        };
        list("only in ezdxf", &c.missing_in_rs);
        list("only in cadcraft", &c.extra_in_rs);
        list("layer table", &c.layer_notes);
        list("block table", &c.block_notes);
    }
}

fn floors_of(t: &Totals) -> Value {
    let by: BTreeMap<&String, usize> = t.by_type.iter().map(|(k, s)| (k, s.agree)).collect();
    json!({
        "comment": "Minimum agreement counts for CI. Generated by `dxf_oracle report --write-floors`; raise them when a fix lands, never lower them to pass.",
        "ezdxf": "1.4.5",
        "files": t.files,
        "both_parse_min": t.both,
        "entities_agree_min": t.agree,
        "agree_by_type_min": by,
        "roundtrip_equal_files_min": t.rt_equal,
        "export_audit_clean_files_min": t.audit_clean,
        "export_audit_errors_max": t.audit_errors,
    })
}

fn check_floors(t: &Totals, floors: &Value) -> Vec<String> {
    let mut bad = Vec::new();
    let mut min = |name: &str, got: usize| {
        let want = floors[name].as_u64().unwrap_or(0) as usize;
        if got < want {
            bad.push(format!("{name}: {got} < floor {want}"));
        }
    };
    min("both_parse_min", t.both);
    min("entities_agree_min", t.agree);
    min("roundtrip_equal_files_min", t.rt_equal);
    min("export_audit_clean_files_min", t.audit_clean);
    if let Some(m) = floors["agree_by_type_min"].as_object() {
        for (k, v) in m {
            let got = t.by_type.get(k).map_or(0, |s| s.agree);
            let want = v.as_u64().unwrap_or(0) as usize;
            if got < want {
                bad.push(format!("agree[{k}]: {got} < floor {want}"));
            }
        }
    }
    let max = floors["export_audit_errors_max"].as_u64().unwrap_or(u64::MAX) as usize;
    if t.audit_errors > max {
        bad.push(format!("export_audit_errors: {} > ceiling {max}", t.audit_errors));
    }
    bad
}

fn markdown(rows: &[Row], t: &Totals, o: &Opts) -> String {
    let mut md = String::new();
    let ezv = "ezdxf";
    let _ = writeln!(md, "# DXF oracle: cadcraft against {ezv}\n");
    let _ = writeln!(md, "<!-- Generated by tools/dxf-oracle/run.sh (dxf_oracle report). Edit tools/dxf-oracle/findings.md, not this file. -->\n");
    let _ = writeln!(
        md,
        "Ticket cloudbring/artcraft-algorithms#196 (V4.3). Evidence level L4 for the claims below once this runs in CI (independent oracle: ezdxf shares no code with cadcraft). See `tools/dxf-oracle/README.md` for the method.\n"
    );
    let _ = writeln!(md, "## Totals\n");
    let _ = writeln!(md, "| | |\n|---|---:|");
    let _ = writeln!(md, "| Files | {} |", t.files);
    let _ = writeln!(md, "| ezdxf strict parse ok | {} (recovered with `ezdxf.recover`: {}) |", t.ez_ok, t.ez_recovered);
    let _ = writeln!(md, "| cadcraft parse ok | {} |", t.rs_ok);
    let _ = writeln!(md, "| Parsed by both (entity statistics below cover these) | {} |", t.both);
    let _ = writeln!(md, "| Entities compared (ezdxf entities in those files) | {} |", t.entities);
    let _ = writeln!(
        md,
        "| Matched (by space and handle; {} of them by file order within space and type, for files whose handles differ) | {} |",
        t.by_order, t.matched
    );
    let _ = writeln!(md, "| Of those, compared without geometry (extrusion other than +Z: TEXT and MTEXT in a flipped system) | {} |", t.ocs);
    let _ = writeln!(
        md,
        "| Entities in generated blocks (`*D` dimension, `*T` table, standard arrowheads), left out: ezdxf / cadcraft | {} / {} |",
        t.generated.0, t.generated.1
    );
    let _ = writeln!(
        md,
        "| Agree (type, layer, colour, linetype, lineweight and geometry within {:e} relative) | {} |",
        crate::compare::REL_TOL,
        t.agree
    );
    let _ = writeln!(md, "| **Agreement** | **{}** |", pct(t.agree, t.entities));
    let _ = writeln!(md, "| Entities missing in cadcraft / only in cadcraft | {} / {} |", t.missing, t.extra);
    let _ = writeln!(md, "| DXF version ($ACADVER) equal | {} of {} |", t.version_equal, t.both);
    let _ = writeln!(md, "| Files with a layer table difference | {} |", t.layer_note_files);
    let _ = writeln!(md, "| Files with a block table difference | {} |", t.block_note_files);
    let _ = writeln!(md, "| ezdxf `audit()` errors on the original files | {} |", t.audit_errors_orig);
    let _ = writeln!(md, "\n## By entity type\n");
    type_table(&mut md, &t.by_type, "ezdxf", "cadcraft");
    let _ = writeln!(md, "\n## Entity types cadcraft keeps as unknown (`Unknown`, passed through, not interpreted)\n");
    if t.unsupported.is_empty() {
        let _ = writeln!(md, "None.");
    } else {
        let _ = writeln!(md, "| DXF type | Entities | Files |\n|---|---:|---:|");
        for (k, (n, f)) in &t.unsupported {
            let _ = writeln!(md, "| {k} | {n} | {f} |");
        }
    }
    let _ = writeln!(md, "\n## Where they disagree (fields, most frequent first)\n");
    if t.fields.0.is_empty() {
        let _ = writeln!(md, "No disagreements.");
    } else {
        fields_table(&mut md, &t.fields, 40);
    }
    if t.layer_fields.total() > 0 {
        let _ = writeln!(md, "\nLayer table fields:\n");
        fields_table(&mut md, &t.layer_fields, 20);
    }
    let _ = writeln!(md, "\n## Round trip: cadcraft import, export DXF, ezdxf audit, re-import\n");
    let _ = writeln!(md, "| | |\n|---|---:|");
    let _ = writeln!(md, "| Files round-tripped | {} |", t.rt_ran);
    let _ = writeln!(md, "| Re-imported dump equals the first (entities by handle, layers, blocks) | {} |", t.rt_equal);
    let _ = writeln!(md, "| Exports audited by ezdxf | {} |", t.audit_files);
    let _ = writeln!(md, "| Exports with zero audit errors | {} |", t.audit_clean);
    let _ = writeln!(md, "| Audit errors on exports (total) | {} |", t.audit_errors);
    let _ = writeln!(md, "\nezdxf's reading of cadcraft's export against ezdxf's reading of the original (the writer, seen from outside):\n");
    type_table(&mut md, &t.writer, "original", "export");
    if t.writer_fields.total() > 0 {
        let _ = writeln!(md, "\nWhere the export differs from the original, as ezdxf reads it:\n");
        fields_table(&mut md, &t.writer_fields, 25);
    }
    let _ = writeln!(md, "\n## Files that did not round-trip cleanly\n");
    let mut any = false;
    for r in rows.iter().filter(|r| r.trip.ran && (!r.trip.stage_failed.is_empty() || !r.trip.equal || r.trip.audit_errors > 0)) {
        any = true;
        let why = if !r.trip.stage_failed.is_empty() {
            format!("failed at {}: {}", r.trip.stage_failed, r.trip.error.chars().take(100).collect::<String>())
        } else if r.trip.audit_errors > 0 {
            format!("{} audit error(s): {}", r.trip.audit_errors, r.trip.audit_messages.join(" / ").chars().take(160).collect::<String>())
        } else {
            "re-imported dump differs".to_string()
        };
        let _ = writeln!(md, "- `{}`: {}", r.cmp.file, why.replace('\n', " "));
    }
    if !any {
        let _ = writeln!(md, "None.");
    }
    if let Some(f) = &o.findings
        && let Ok(text) = std::fs::read_to_string(f)
    {
        let _ = writeln!(md, "\n{text}");
    }
    let floors = floors_of(t);
    let _ = writeln!(
        md,
        "\n## Floors\n\nEnforced by `tools/dxf-oracle/run.sh --check` against `tools/dxf-oracle/floors.json` (counts, not percentages, so a regression of even one entity shows):\n"
    );
    let _ = writeln!(md, "| Floor | Value |\n|---|---:|");
    for k in ["both_parse_min", "entities_agree_min", "roundtrip_equal_files_min", "export_audit_clean_files_min", "export_audit_errors_max"] {
        let _ = writeln!(md, "| {k} | {} |", floors[k]);
    }
    let _ = writeln!(md, "\nPer-type agreement floors are in `floors.json` (`agree_by_type_min`).");
    let _ = writeln!(md, "\n## Differences by file (examples)\n");
    details(&mut md, rows);
    let _ = writeln!(md, "\n## Per file\n");
    per_file(&mut md, rows);
    md
}

pub fn run(o: &Opts) -> Result<i32, String> {
    let mut names: Vec<String> = std::fs::read_dir(&o.ez)
        .map_err(|e| format!("{}: {e}", o.ez.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    let mut rows = Vec::new();
    for n in names.iter().filter(|n| n.ends_with(".json")) {
        let ez = load(&o.ez, n);
        let rs = load(&o.rs, n);
        let file = ez["file"].as_str().unwrap_or(n).to_string();
        let cmp = compare_file(&file, &ez, &rs);
        let trip = trip_of(n, &ez, o);
        rows.push(Row { cmp, trip });
    }
    let t = totals(&rows);
    std::fs::write(&o.md, markdown(&rows, &t, o)).map_err(|e| e.to_string())?;
    println!(
        "files {} both-parse {} entities {} agree {} ({}) roundtrip-equal {}/{} export-audit-errors {}",
        t.files,
        t.both,
        t.entities,
        t.agree,
        pct(t.agree, t.entities),
        t.rt_equal,
        t.rt_ran,
        t.audit_errors
    );
    if o.write_floors {
        let path = o.floors.as_ref().ok_or("--write-floors needs --floors")?;
        let text = serde_json::to_string_pretty(&floors_of(&t)).map_err(|e| e.to_string())?;
        std::fs::write(path, text + "\n").map_err(|e| e.to_string())?;
    }
    if o.check {
        let path = o.floors.as_ref().ok_or("--check needs --floors")?;
        let floors: Value = serde_json::from_str(&std::fs::read_to_string(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        let bad = check_floors(&t, &floors);
        for b in &bad {
            eprintln!("FLOOR VIOLATED: {b}");
        }
        if !bad.is_empty() {
            return Ok(1);
        }
        println!("all floors hold");
    }
    Ok(0)
}
