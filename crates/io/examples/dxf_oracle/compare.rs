//! Comparing two canonical dumps. The first (ezdxf) is the reference: every key it has must agree;
//! keys only the second has are ignored unless the comparison is symmetric (round trips).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde_json::Value;

/// Relative tolerance of every number, with an absolute floor of the same size for values near 0.
pub const REL_TOL: f64 = 1e-9;

pub fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= REL_TOL * a.abs().max(b.abs()).max(1.0)
}

fn close_angle(a: f64, b: f64) -> bool {
    let d = (a - b).rem_euclid(360.0);
    let d = if d > 180.0 { 360.0 - d } else { d };
    d <= REL_TOL * 360.0
}

/// How often each field disagreed, with one example of the values.
#[derive(Default, Clone)]
pub struct Fields(pub BTreeMap<String, (usize, String)>);

impl Fields {
    fn note(&mut self, path: &str, example: impl FnOnce() -> String) {
        let slot = self.0.entry(path.to_string()).or_insert_with(|| (0, example()));
        slot.0 += 1;
    }
    pub fn merge(&mut self, other: &Fields) {
        for (k, (n, ex)) in &other.0 {
            let slot = self.0.entry(k.clone()).or_insert_with(|| (0, ex.clone()));
            slot.0 += n;
        }
    }
    pub fn total(&self) -> usize {
        self.0.values().map(|v| v.0).sum()
    }
}

fn short(v: &Value) -> String {
    let s = v.to_string();
    if s.len() > 90 { format!("{}...", s.chars().take(90).collect::<String>()) } else { s }
}

/// Compare `a` (reference) with `b`, noting each disagreeing field path in `out`.
pub fn diff(a: &Value, b: &Value, path: &str, sym: bool, out: &mut Fields) {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            let (x, y) = (x.as_f64().unwrap_or(f64::NAN), y.as_f64().unwrap_or(f64::NAN));
            let ok = if path.ends_with("_deg") { close_angle(x, y) } else { close(x, y) };
            if !ok {
                out.note(path, || format!("ref={x} got={y}"));
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                out.note(&format!("{path}[len]"), || format!("ref={} got={}", x.len(), y.len()));
                return;
            }
            let p = format!("{path}[]");
            for (u, v) in x.iter().zip(y) {
                diff(u, v, &p, sym, out);
            }
        }
        (Value::Object(x), Value::Object(y)) => {
            for (k, u) in x {
                let p = if path.is_empty() { k.clone() } else { format!("{path}.{k}") };
                match y.get(k) {
                    Some(v) => diff(u, v, &p, sym, out),
                    None if u.is_null() => {}
                    None => out.note(&p, || "missing in got".to_string()),
                }
            }
            if sym {
                for k in y.keys().filter(|k| !x.contains_key(*k)) {
                    out.note(&format!("{path}.{k}"), || "extra in got".to_string());
                }
            }
        }
        _ if a == b => {}
        _ => out.note(path, || format!("ref={} got={}", short(a), short(b))),
    }
}

/// An entity's identity: where it lives and its handle.
type Key = (String, String);

fn key(e: &Value) -> Key {
    let s = |k: &str| e.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    (s("space"), s("handle"))
}

fn ty(e: &Value) -> String {
    e.get("type").and_then(Value::as_str).unwrap_or("?").to_string()
}

/// Per entity type: counts on each side and how many reference entities agree completely.
#[derive(Default, Clone)]
pub struct TypeStat {
    pub ez: usize,
    pub rs: usize,
    pub matched: usize,
    pub agree: usize,
}

/// The result of comparing one file.
#[derive(Default, Clone)]
pub struct FileCmp {
    pub file: String,
    pub ez_ok: bool,
    pub ez_recovered: bool,
    pub ez_error: String,
    pub rs_ok: bool,
    pub rs_error: String,
    pub version: (String, String),
    pub by_type: BTreeMap<String, TypeStat>,
    pub missing_in_rs: Vec<String>,
    pub extra_in_rs: Vec<String>,
    pub entity_fields: Fields,
    pub layer_fields: Fields,
    pub block_notes: Vec<String>,
    pub layer_notes: Vec<String>,
    pub unsupported: BTreeMap<String, usize>,
    pub audit_errors: usize,
    /// Entities with a non-default extrusion, compared without geometry.
    pub ocs_skipped: usize,
    /// Entities paired in file order because their handles did not match.
    pub by_order: usize,
    /// Entities of generated blocks left out of the statistics, by side.
    pub generated: (usize, usize),
}

impl FileCmp {
    /// Both sides parsed the file, so its entity statistics count.
    pub fn compared(&self) -> bool {
        self.ez_ok && self.rs_ok
    }
    pub fn ez_entities(&self) -> usize {
        self.by_type.values().map(|t| t.ez).sum()
    }
    pub fn agree(&self) -> usize {
        self.by_type.values().map(|t| t.agree).sum()
    }
    pub fn matched(&self) -> usize {
        self.by_type.values().map(|t| t.matched).sum()
    }
}

fn a_has_ocs(e: &Value) -> bool {
    e.get("ocs").is_some()
}

fn entity_diff(a: &Value, b: &Value, ctx: &str, fields: &mut Fields) -> bool {
    let before = fields.total();
    let t = ty(a);
    let mut f = Fields::default();
    if ty(a) != ty(b) {
        f.note("type", || format!("ref={} got={}", ty(a), ty(b)));
    }
    for k in ["layer", "color", "lineweight"] {
        diff(&a[k], &b[k], k, false, &mut f);
    }
    let lt = |v: &Value| v.as_str().unwrap_or("").to_ascii_lowercase();
    if lt(&a["linetype"]) != lt(&b["linetype"]) {
        f.note("linetype", || format!("ref={} got={}", a["linetype"], b["linetype"]));
    }
    // Entities in a flipped coordinate system (extrusion other than +Z) are compared on type,
    // layer and properties only: the oracle side does not re-derive cadcraft's WCS form.
    if a.get("ocs").is_none() {
        diff(&a["geom"], &b["geom"], "geom", false, &mut f);
    }
    for (k, (n, ex)) in f.0 {
        let slot = fields.0.entry(format!("{t}.{k}")).or_insert_with(|| (0, format!("{ctx}: {ex}")));
        slot.0 += n;
    }
    fields.total() != before
}

/// Standard dimension arrowhead blocks, which AutoCAD writes whether or not a drawing uses them.
const ARROW_BLOCKS: [&str; 20] = [
    "_ARCHTICK",
    "_BOXBLANK",
    "_BOXFILLED",
    "_CLOSED",
    "_CLOSEDBLANK",
    "_CLOSEDFILLED",
    "_DATUMBLANK",
    "_DATUMFILLED",
    "_DOT",
    "_DOTBLANK",
    "_DOTSMALL",
    "_INTEGRAL",
    "_NONE",
    "_OBLIQUE",
    "_OPEN",
    "_OPEN30",
    "_OPEN90",
    "_ORIGIN",
    "_ORIGIN2",
    "_SMALL",
];

/// Blocks a program generates from other objects (dimension `*D`, table `*T`, standard arrowheads):
/// cadcraft rebuilds them on save, so their contents are not compared.
pub fn is_generated(space: &str) -> bool {
    let Some(name) = space.strip_prefix("block:") else { return false };
    let u = name.to_ascii_uppercase();
    u.starts_with("*D") || u.starts_with("*T") || ARROW_BLOCKS.contains(&u.as_str())
}

fn compare_pair(e: &Value, r: &Value, c: &mut FileCmp) {
    let t = ty(e);
    c.by_type.entry(t.clone()).or_default().matched += 1;
    let ctx = format!("{}#{}", c.file, key(e).1);
    if a_has_ocs(e) {
        c.ocs_skipped += 1;
    }
    if !entity_diff(e, r, &ctx, &mut c.entity_fields) {
        c.by_type.entry(t).or_default().agree += 1;
    }
}

fn compare_entities(ez: &Value, rs: &Value, c: &mut FileCmp) {
    let empty = Vec::new();
    let all_ez = ez["entities"].as_array().unwrap_or(&empty);
    let all_rs = rs["entities"].as_array().unwrap_or(&empty);
    let ez_es: Vec<&Value> = all_ez.iter().filter(|e| !is_generated(&key(e).0)).collect();
    let rs_es: Vec<&Value> = all_rs.iter().filter(|e| !is_generated(&key(e).0)).collect();
    c.generated = (all_ez.len() - ez_es.len(), all_rs.len() - rs_es.len());
    let rs_by: HashMap<Key, &Value> = rs_es.iter().map(|e| (key(e), *e)).collect();
    let mut used: BTreeSet<Key> = BTreeSet::new();
    for e in &rs_es {
        let t = ty(e);
        c.by_type.entry(t.clone()).or_default().rs += 1;
        if e.get("unsupported").is_some() {
            *c.unsupported.entry(t).or_default() += 1;
        }
    }
    let mut left_ez: Vec<&Value> = Vec::new();
    for e in &ez_es {
        c.by_type.entry(ty(e)).or_default().ez += 1;
        match rs_by.get(&key(e)) {
            Some(r) => {
                used.insert(key(e));
                compare_pair(e, r, c);
            }
            None => left_ez.push(e),
        }
    }
    // Files without handles (R12 and older) or with duplicate ones are numbered differently by every
    // program: the entities left over are paired in file order within (space, type).
    let mut left_rs: HashMap<(String, String), std::collections::VecDeque<&Value>> = HashMap::new();
    for e in rs_es.iter().filter(|e| !used.contains(&key(e))) {
        left_rs.entry((key(e).0, ty(e))).or_default().push_back(e);
    }
    for e in left_ez {
        match left_rs.get_mut(&(key(e).0, ty(e))).and_then(std::collections::VecDeque::pop_front) {
            Some(r) => {
                c.by_order += 1;
                compare_pair(e, r, c);
            }
            None => c.missing_in_rs.push(format!("{} {}:{}", ty(e), key(e).0, key(e).1)),
        }
    }
    for e in left_rs.values().flatten() {
        c.extra_in_rs.push(format!("{} {}:{}", ty(e), key(e).0, key(e).1));
    }
    c.extra_in_rs.sort();
}

fn compare_tables(ez: &Value, rs: &Value, c: &mut FileCmp) {
    let empty = serde_json::Map::new();
    let (zl, rl) = (ez["layers"].as_object().unwrap_or(&empty), rs["layers"].as_object().unwrap_or(&empty));
    for (name, a) in zl {
        match rl.get(name) {
            Some(b) => diff(a, b, "layer", false, &mut c.layer_fields),
            None => c.layer_notes.push(format!("layer {name:?} missing in cadcraft")),
        }
    }
    for name in rl.keys().filter(|n| !zl.contains_key(*n)) {
        c.layer_notes.push(format!("layer {name:?} only in cadcraft"));
    }
    let (zb, rb) = (ez["blocks"].as_object().unwrap_or(&empty), rs["blocks"].as_object().unwrap_or(&empty));
    let gen_name = |n: &String| is_generated(&format!("block:{n}"));
    for (name, a) in zb.iter().filter(|(n, _)| !gen_name(n)) {
        match rb.get(name) {
            Some(b) => {
                for k in ["base", "count"] {
                    let mut f = Fields::default();
                    diff(&a[k], &b[k], k, false, &mut f);
                    c.block_notes.extend(f.0.into_iter().map(|(p, (_, ex))| format!("block {name:?} {p}: {ex}")));
                }
            }
            None => c.block_notes.push(format!("block {name:?} missing in cadcraft")),
        }
    }
    for name in rb.keys().filter(|n| !zb.contains_key(*n) && !gen_name(n)) {
        c.block_notes.push(format!("block {name:?} only in cadcraft"));
    }
    if ez["paper_layouts"] != rs["paper_layouts"] {
        c.block_notes.push(format!("paper layouts: ref={} got={}", ez["paper_layouts"], rs["paper_layouts"]));
    }
}

/// Compare one file's two dumps.
pub fn compare_file(file: &str, ez: &Value, rs: &Value) -> FileCmp {
    let mut c = FileCmp { file: file.to_string(), ..Default::default() };
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    c.ez_ok = ez["ok"].as_bool().unwrap_or(false);
    c.ez_recovered = ez["recovered"].as_bool().unwrap_or(false);
    c.ez_error = s(ez, "error");
    c.rs_ok = rs["ok"].as_bool().unwrap_or(false);
    c.rs_error = s(rs, "error");
    c.audit_errors = ez["audit"]["errors"].as_array().map_or(0, Vec::len);
    c.version = (s(ez, "version"), s(rs, "version"));
    if c.rs_ok && ez.get("entities").is_some() {
        // Recovered files are compared too, but their statistics are kept apart by `compared()`.
        compare_entities(ez, rs, &mut c);
        compare_tables(ez, rs, &mut c);
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fields(a: &Value, b: &Value) -> Fields {
        let mut f = Fields::default();
        diff(a, b, "", false, &mut f);
        f
    }

    #[test]
    fn numbers_agree_within_the_relative_tolerance_only() {
        assert_eq!(fields(&json!({"r": 1000.0}), &json!({"r": 1000.0 + 5e-7})).total(), 0);
        assert_eq!(fields(&json!({"r": 1000.0}), &json!({"r": 1000.0 + 5e-5})).total(), 1);
        assert_eq!(fields(&json!({"r": 0.0}), &json!({"r": 5e-10})).total(), 0);
    }

    #[test]
    fn angles_compare_modulo_a_turn_and_missing_keys_are_noted() {
        assert_eq!(fields(&json!({"a_deg": 359.9999999999}), &json!({"a_deg": -1e-10})).total(), 0);
        assert_eq!(fields(&json!({"a_deg": 90.0}), &json!({"a_deg": 91.0})).total(), 1);
        assert_eq!(fields(&json!({"p": [1.0, 2.0]}), &json!({})).0.keys().collect::<Vec<_>>(), ["p"]);
        assert_eq!(fields(&json!({"p": [1.0, 2.0]}), &json!({"p": [1.0]})).0.keys().collect::<Vec<_>>(), ["p[len]"]);
    }

    #[test]
    fn dimension_and_table_blocks_are_generated() {
        assert!(is_generated("block:*D12"));
        assert!(is_generated("block:*T1"));
        assert!(is_generated("block:_dotsmall"));
        assert!(!is_generated("block:*U3"));
        assert!(!is_generated("block:DOOR"));
        assert!(!is_generated("model"));
    }

    fn file(handles: &[&str]) -> Value {
        let es: Vec<Value> = handles
            .iter()
            .map(|h| {
                json!({"space": "model", "type": "LINE", "handle": h, "layer": "0", "color": 256, "linetype": "BYLAYER", "lineweight": -1,
                "geom": {"a": [0.0, 0.0, 0.0], "b": [1.0, 0.0, 0.0]}})
            })
            .collect();
        json!({"ok": true, "recovered": false, "version": "AC1009", "layers": {}, "blocks": {}, "paper_layouts": [], "entities": es})
    }

    #[test]
    fn entities_with_other_handles_are_paired_in_order() {
        let c = compare_file("x.dxf", &file(&["4", "5", "6"]), &file(&["100", "101", "102"]));
        assert_eq!((c.matched(), c.agree(), c.by_order), (3, 3, 3));
        let c = compare_file("x.dxf", &file(&["4", "5", "6"]), &file(&["4", "5"]));
        assert_eq!((c.matched(), c.missing_in_rs.len(), c.by_order), (2, 1, 0));
    }
}
