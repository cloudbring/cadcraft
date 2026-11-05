//! The Rust side of the ezdxf oracle (`tools/dxf-oracle/`).
//!
//! ```text
//! dxf_oracle dump      OUT_DIR ROOT REL...   cadcraft's canonical JSON per file
//! dxf_oracle roundtrip OUT_DIR ROOT REL...   import -> export DXF -> re-import, per file
//! dxf_oracle report    --ez DIR --rs DIR --rt DIR --audit DIR --md FILE [--floors FILE] [--check] [--write-floors]
//! ```
//!
//! `tools/dxf-oracle/run.sh` runs all of it, with the Python side.

mod compare;
mod dump;
mod report;

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

fn flat(rel: &str) -> String {
    format!("{}.json", rel.replace('/', "__"))
}

fn write_json(dir: &Path, rel: &str, v: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(v).map_err(|e| e.to_string())?;
    std::fs::write(dir.join(flat(rel)), text + "\n").map_err(|e| e.to_string())
}

/// Read a file, turning a panic of the importer into an error result (a finding, not a crash).
fn read_guarded(bytes: &[u8], rel: &str) -> Result<cadcraft_doc::Drawing, String> {
    let rel = rel.to_string();
    let bytes = bytes.to_vec();
    match std::panic::catch_unwind(move || cadcraft_io::read(&bytes, &rel)) {
        Ok(Ok(d)) => Ok(d),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("PANIC in the importer".to_string()),
    }
}

fn dump_one(root: &Path, rel: &str) -> Value {
    match std::fs::read(root.join(rel)) {
        Err(e) => json!({"tool": "cadcraft", "ok": false, "error": format!("read: {e}")}),
        Ok(bytes) => match read_guarded(&bytes, rel) {
            Ok(d) => {
                let mut v = dump::dump(&d);
                v["file"] = json!(rel);
                v
            }
            Err(e) => json!({"tool": "cadcraft", "file": rel, "ok": false, "error": e}),
        },
    }
}

fn cmd_dump(out: &Path, root: &Path, rels: &[String]) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    for rel in rels {
        write_json(out, rel, &dump_one(root, rel))?;
    }
    Ok(())
}

/// Import, export, re-import: the second dump must equal the first.
fn roundtrip_one(root: &Path, rel: &str, dxf_dir: &Path) -> Value {
    let fail = |stage: &str, e: String| json!({"file": rel, "ok": false, "stage": stage, "error": e});
    let bytes = match std::fs::read(root.join(rel)) {
        Ok(b) => b,
        Err(e) => return fail("read", e.to_string()),
    };
    let d1 = match read_guarded(&bytes, rel) {
        Ok(d) => d,
        Err(e) => return fail("import", e),
    };
    let out = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cadcraft_io::write(&d1, "out.dxf"))) {
        Ok(Ok(b)) => b,
        Ok(Err(e)) => return fail("export", e.to_string()),
        Err(_) => return fail("export", "PANIC in the exporter".into()),
    };
    let saved = dxf_dir.join(rel.replace('/', "__"));
    if let Err(e) = std::fs::write(&saved, &out) {
        return fail("save", e.to_string());
    }
    let d2 = match read_guarded(&out, rel) {
        Ok(d) => d,
        Err(e) => return fail("reimport", e),
    };
    let (a, b) = (dump::dump(&d1), dump::dump(&d2));
    let c = compare::compare_file(rel, &a, &b);
    let equal = c.agree() == c.ez_entities()
        && c.extra_in_rs.is_empty()
        && c.layer_notes.is_empty()
        && c.block_notes.is_empty()
        && c.layer_fields.total() == 0;
    json!({
        "file": rel, "ok": true, "equal": equal, "entities": c.ez_entities(), "agree": c.agree(),
        "missing": c.missing_in_rs.len(), "extra": c.extra_in_rs.len(), "layer_notes": c.layer_notes.len(), "block_notes": c.block_notes.len(),
        "fields": report::fields_json(&c.entity_fields), "notes": c.layer_notes.iter().chain(c.block_notes.iter()).take(8).collect::<Vec<_>>(),
        "missing_examples": c.missing_in_rs.iter().take(5).collect::<Vec<_>>(),
    })
}

fn cmd_roundtrip(out: &Path, root: &Path, rels: &[String]) -> Result<(), String> {
    let dxf_dir = out.join("dxf");
    std::fs::create_dir_all(&dxf_dir).map_err(|e| e.to_string())?;
    for rel in rels {
        write_json(out, rel, &roundtrip_one(root, rel, &dxf_dir))?;
    }
    Ok(())
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn run(args: &[String]) -> Result<i32, String> {
    let usage = "usage: dxf_oracle dump|roundtrip OUT_DIR ROOT REL... | report --ez DIR --rs DIR --rt DIR --audit DIR --md FILE [--floors FILE] [--check] [--write-floors]";
    let cmd = args.first().ok_or(usage)?;
    match cmd.as_str() {
        "dump" | "roundtrip" => {
            let (out, root) = (args.get(1).ok_or(usage)?, args.get(2).ok_or(usage)?);
            let rels = args.get(3..).unwrap_or(&[]);
            if cmd == "dump" {
                cmd_dump(Path::new(out), Path::new(root), rels)?;
            } else {
                cmd_roundtrip(Path::new(out), Path::new(root), rels)?;
            }
            Ok(0)
        }
        "report" => {
            let need = |n: &str| flag(args, n).map(PathBuf::from).ok_or(format!("report needs {n}"));
            let opts = report::Opts {
                ez: need("--ez")?,
                rs: need("--rs")?,
                rt: need("--rt")?,
                audit: need("--audit")?,
                md: need("--md")?,
                floors: flag(args, "--floors").map(PathBuf::from),
                check: args.iter().any(|a| a == "--check"),
                write_floors: args.iter().any(|a| a == "--write-floors"),
                findings: flag(args, "--findings").map(PathBuf::from),
            };
            report::run(&opts)
        }
        _ => Err(usage.into()),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("dxf_oracle: {e}");
            std::process::exit(2);
        }
    }
}
