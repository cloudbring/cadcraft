//! Sketch generators. Everything is built on the NEW crate's types, then reduced to neutral JSON.
use std::path::Path;

use new_doc::geom::Vec3;
use new_doc::{Circle, Common, Constraint, ConstraintKind, DistAxis, Drawing, EntityKind, GeomRef, Line, Point as DPoint, Space, Sub};
use crate::Sketch;

/// Real drawings are cut to their first ENTITY_CAP supported entities (drawing order).
const ENTITY_CAP: usize = 1200;
/// Auto-constraining is run on consecutive chunks of this many entities: `autoconstrain` solves and
/// rank-checks the whole component after every candidate, which is far too slow on one big set.
const CHUNK: usize = 40;
pub const SIZES: [usize; 6] = [50, 200, 500, 1000, 2000, 5000];

struct Rng(u64);
impl Rng {
    fn new(seed: &str) -> Self {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for b in seed.bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
        Rng(h | 1)
    }
    /// Uniform in [-1, 1].
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 52) as f64 - 1.0
    }
}

fn neutral(name: String, group: &str, d: &Drawing, kinds: &[EntityKind]) -> Sketch {
    Sketch {
        name,
        group: group.into(),
        ents: kinds.iter().map(|k| serde_json::to_value(k).expect("entity json")).collect(),
        cons: d.constraints.iter().map(|c| serde_json::to_value(c).expect("constraint json")).collect(),
    }
}

fn add(d: &mut Drawing, kinds: &mut Vec<EntityKind>, k: EntityKind) -> new_doc::Handle {
    kinds.push(k.clone());
    d.add(&Space::Model, Common::default(), k).expect("add")
}

fn con(d: &mut Drawing, kind: ConstraintKind, refs: Vec<GeomRef>, expr: &str) {
    let id = d.constraints.len() as u32;
    d.constraints.push(Constraint { id, kind, refs, name: String::new(), expr: expr.into() });
}

/// Verbatim shape of the benchmark in crates/constraints/src/bench.rs (#16): a point chain with a
/// horizontal and a vertical Distance per link; `rows` constraints, one residual row each.
fn chain(rows: usize) -> (Drawing, Vec<EntityKind>) {
    let (mut d, mut kinds) = (Drawing::default(), Vec::new());
    let mut prev = None;
    for i in 0..=rows / 2 {
        let y = 0.05 * (i as f64 * 0.37).sin();
        let h = add(&mut d, &mut kinds, EntityKind::Point(DPoint { p: Vec3::new(i as f64 * 2.0, y, 0.0), angle: 0.0 }));
        if let Some(p) = prev {
            for (axis, target) in [(DistAxis::Horizontal, "2"), (DistAxis::Vertical, "0")] {
                con(&mut d, ConstraintKind::Distance(axis), vec![GeomRef::whole(p), GeomRef::whole(h)], target);
            }
        }
        prev = Some(h);
    }
    (d, kinds)
}

/// k x k lattice of points, horizontal and vertical Distance on every edge, truncated to `n`.
fn grid(n: usize) -> (Drawing, Vec<EntityKind>) {
    let mut k = 2;
    while 2 * k * (k - 1) < n {
        k += 1;
    }
    let (mut d, mut kinds) = (Drawing::default(), Vec::new());
    let mut hs = Vec::new();
    for j in 0..k {
        for i in 0..k {
            let x = i as f64 * 2.0 + 0.05 * ((i + 3 * j) as f64 * 0.37).sin();
            let y = j as f64 * 2.0 + 0.05 * ((i * j) as f64 * 0.53).cos();
            hs.push(add(&mut d, &mut kinds, EntityKind::Point(DPoint { p: Vec3::new(x, y, 0.0), angle: 0.0 })));
        }
    }
    for j in 0..k {
        for i in 0..k {
            if i + 1 < k {
                con(&mut d, ConstraintKind::Distance(DistAxis::Horizontal), vec![GeomRef::whole(hs[j * k + i]), GeomRef::whole(hs[j * k + i + 1])], "2");
            }
            if j + 1 < k {
                con(&mut d, ConstraintKind::Distance(DistAxis::Vertical), vec![GeomRef::whole(hs[j * k + i]), GeomRef::whole(hs[(j + 1) * k + i])], "2");
            }
        }
    }
    d.constraints.truncate(n);
    (d, kinds)
}

/// A joined path of lines in groups of six plus one circle per group, carrying a random-looking
/// mix of constraint kinds that the true geometry satisfies. `cut_every`: break the path (new
/// connected component) every that many groups; 0 = one component.
fn mix(n: usize, cut_every: usize) -> (Drawing, Vec<EntityKind>) {
    let (mut d, mut kinds) = (Drawing::default(), Vec::new());
    let groups = n.div_ceil(14).max(1);
    let mut pos = (0.0f64, 0.0f64);
    let mut prev_line: Option<new_doc::Handle> = None;
    for g in 0..groups {
        let th = 0.3 + 0.5 * (g % 5) as f64;
        let len: Vec<f64> = (0..6).map(|i| 3.0 + ((g * 6 + i) * 7 % 5) as f64).collect();
        let len = [len[0], len[0], len[2], len[3], len[4], len[5]];
        let dirs = [(1.0, 0.0), (0.0, 1.0), (th.cos(), th.sin()), (th.cos(), th.sin()), (-th.sin(), th.cos()), ((th + 0.9).cos(), (th + 0.9).sin())];
        let mut ls = Vec::new();
        let mut start2 = (0.0, 0.0);
        if cut_every > 0 && g % cut_every == 0 {
            prev_line = None;
            pos = (g as f64 * 40.0, 0.0);
        }
        for i in 0..6 {
            let a = pos;
            let b = (a.0 + dirs[i].0 * len[i], a.1 + dirs[i].1 * len[i]);
            let h = add(&mut d, &mut kinds, EntityKind::Line(Line { a: Vec3::new(a.0, a.1, 0.0), b: Vec3::new(b.0, b.1, 0.0) }));
            if let Some(p) = prev_line {
                con(&mut d, ConstraintKind::Coincident, vec![GeomRef::new(p, Sub::End), GeomRef::new(h, Sub::Start)], "");
            }
            if i == 2 {
                start2 = a;
            }
            prev_line = Some(h);
            ls.push(h);
            pos = b;
        }
        let circ = add(&mut d, &mut kinds, EntityKind::Circle(Circle { center: Vec3::new(start2.0, start2.1, 0.0), radius: 1.5 }));
        let w = GeomRef::whole;
        con(&mut d, ConstraintKind::Horizontal, vec![w(ls[0])], "");
        con(&mut d, ConstraintKind::Vertical, vec![w(ls[1])], "");
        con(&mut d, ConstraintKind::Parallel, vec![w(ls[3]), w(ls[2])], "");
        con(&mut d, ConstraintKind::Perpendicular, vec![w(ls[4]), w(ls[2])], "");
        con(&mut d, ConstraintKind::Distance(DistAxis::Aligned), vec![GeomRef::new(ls[5], Sub::Start), GeomRef::new(ls[5], Sub::End)], &format!("{}", len[5]));
        con(&mut d, ConstraintKind::Equal, vec![w(ls[0]), w(ls[1])], "");
        con(&mut d, ConstraintKind::Coincident, vec![GeomRef::new(circ, Sub::Center), GeomRef::new(ls[2], Sub::Start)], "");
        con(&mut d, ConstraintKind::Radius, vec![w(circ)], "1.5");
    }
    d.constraints.truncate(n);
    // Renumber ids after truncation is unnecessary (ids are 0..n already).
    (d, kinds)
}

fn jitter(kinds: &[EntityKind], amp: f64, seed: &str) -> Vec<EntityKind> {
    let mut r = Rng::new(seed);
    let mut j = |v: &mut f64| *v += amp * r.next();
    kinds
        .iter()
        .map(|k| {
            let mut k = k.clone();
            match &mut k {
                EntityKind::Line(l) => {
                    j(&mut l.a.x);
                    j(&mut l.a.y);
                    j(&mut l.b.x);
                    j(&mut l.b.y);
                }
                EntityKind::Point(p) => {
                    j(&mut p.p.x);
                    j(&mut p.p.y);
                }
                EntityKind::Circle(c) => {
                    j(&mut c.center.x);
                    j(&mut c.center.y);
                    j(&mut c.radius);
                }
                EntityKind::Arc(a) => {
                    j(&mut a.center.x);
                    j(&mut a.center.y);
                    j(&mut a.radius);
                }
                EntityKind::LwPolyline(p) => {
                    for v in &mut p.vertices {
                        j(&mut v.p.x);
                        j(&mut v.p.y);
                    }
                }
                _ => {}
            }
            k
        })
        .collect()
}

pub fn synthetic() -> Vec<Sketch> {
    let mut v = Vec::new();
    for &n in &SIZES {
        let (d, k) = chain(n);
        v.push(neutral(format!("syn-chain-{n:04}"), "synthetic", &d, &k));
        let (d, k) = grid(n);
        v.push(neutral(format!("syn-grid-{n:04}"), "synthetic", &d, &k));
        let (d, k) = mix(n, 0);
        let kj = jitter(&k, 0.05, &format!("mix{n}"));
        v.push(neutral(format!("syn-mix-{n:04}"), "synthetic", &d, &kj));
        let (d, k) = mix(n, 2);
        let kj = jitter(&k, 0.05, &format!("clu{n}"));
        v.push(neutral(format!("syn-clustered-{n:04}"), "synthetic", &d, &kj));
    }
    v
}

fn supported(k: &EntityKind) -> bool {
    matches!(k, EntityKind::Line(_) | EntityKind::Arc(_) | EntityKind::Circle(_) | EntityKind::Point(_) | EntityKind::LwPolyline(_))
}

fn size_of(k: &EntityKind) -> Option<f64> {
    match k {
        EntityKind::Line(l) => Some(((l.a.x - l.b.x).powi(2) + (l.a.y - l.b.y).powi(2)).sqrt()),
        EntityKind::Circle(c) => Some(c.radius),
        EntityKind::Arc(a) => Some(a.radius),
        _ => None,
    }
}

/// One DXF (model space, supported kinds) -> exact sketch with auto-constraints, plus a jittered
/// variant. `None` when the file yields fewer than two constraints.
fn from_dxf(name: &str, bytes: &[u8]) -> Vec<Sketch> {
    let Ok(src) = new_io::read_dxf(bytes) else {
        eprintln!("{name}: import failed");
        return vec![];
    };
    let (mut d, mut kinds) = (Drawing::default(), Vec::new());
    let mut hs = Vec::new();
    for e in src.model.iter() {
        if supported(&e.kind) && kinds.len() < ENTITY_CAP {
            hs.push(add(&mut d, &mut kinds, e.kind.clone()));
        }
    }
    if hs.is_empty() {
        return vec![];
    }
    let t = std::time::Instant::now();
    let mut added = 0;
    for chunk in hs.chunks(CHUNK) {
        // `autoconstrain` = `infer` + `add` per candidate; inlined here to put a time budget on a chunk.
        let t_chunk = std::time::Instant::now();
        for c in new_constraints::infer(&d, chunk, &new_constraints::InferOptions::default()) {
            if t_chunk.elapsed() > std::time::Duration::from_millis(1500) {
                break;
            }
            if new_constraints::add(&mut d, c, &new_constraints::SolveOptions::default()).is_ok() {
                added += 1;
            }
        }
    }
    eprintln!("{name}: {} entities, {} constraints ({:?} to infer)", hs.len(), added, t.elapsed());
    if d.constraints.len() < 2 {
        return vec![];
    }
    let mut sizes: Vec<f64> = kinds.iter().filter_map(size_of).filter(|s| *s > 0.0).collect();
    sizes.sort_by(|a, b| a.total_cmp(b));
    let med = sizes.get(sizes.len() / 2).copied().unwrap_or(1.0);
    let kj = jitter(&kinds, 0.01 * med, name);
    let base = name.replace(['/', ' '], "_");
    vec![neutral(format!("{base}@exact"), "real", &d, &kinds), neutral(format!("{base}@jitter"), "real", &d, &kj)]
}

pub fn real(corpus: &Path, apartment: &Path) -> Vec<Sketch> {
    let mut files: Vec<std::path::PathBuf> = Vec::new();
    fn walk(p: &Path, out: &mut Vec<std::path::PathBuf>) {
        if let Ok(rd) = std::fs::read_dir(p) {
            for e in rd.flatten() {
                let q = e.path();
                if q.is_dir() {
                    walk(&q, out);
                } else if q.extension().is_some_and(|x| x.eq_ignore_ascii_case("dxf")) {
                    out.push(q);
                }
            }
        }
    }
    walk(corpus, &mut files);
    files.sort();
    let mut v = Vec::new();
    if let Ok(b) = std::fs::read(apartment) {
        v.extend(from_dxf("cadcraft-apartment.dxf", &b));
    }
    for f in files {
        let rel = f.strip_prefix(corpus).unwrap_or(&f).to_string_lossy().to_string();
        if let Ok(b) = std::fs::read(&f) {
            v.extend(from_dxf(&rel, &b));
        }
    }
    v
}
