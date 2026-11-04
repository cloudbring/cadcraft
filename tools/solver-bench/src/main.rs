//! artcraft-algorithms#191: re-measure the cadcraft constraint solver (#16) with craft-bench.
//!
//! Three builds of the solver are linked side by side:
//!   old  = cadcraft main at the merge-base (git rev), public API only
//!   oldx = old with the 800-row cap and the 4e8 work gate removed (old-uncapped.patch); this is
//!          the dense, finite-difference path the "2,886 ms" figure was taken on
//!   new  = the #16 branch (path)
//! Sketches are built once as neutral JSON (entities + constraints, serde), then rebuilt on each
//! side, so every side sees identical handles, geometry and constraints.
//!
//!   solver-bench run --arm main|uncapped [--corpus DIR] [--out DIR] [--reps N] [--only SUBSTR] [--skip-time]
use std::collections::BTreeMap;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::Instant;

use craft_bench::report::Revs;
use craft_bench::{Config, Paired};
use serde_json::{Value, json};

mod synth;

/// What a timed run returns: cheap to compare, so the timed region holds the solve and nothing else.
#[derive(Debug, Clone, PartialEq)]
pub struct Sum {
    pub ok: bool,
    pub moved: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Flat {
    pub result: String,
    pub skipped: usize,
    pub params: Vec<f64>,
    pub res_pre: f64,
    pub res_post: f64,
    pub satisfied: bool,
    pub dof_pre: String,
    pub dof_post: String,
    pub solve_ms: f64,
}

macro_rules! side {
    ($m:ident, $con:ident, $doc:ident) => {
        pub mod $m {
            use super::{Flat, Sum};
            use std::time::Instant;
            use serde_json::Value;
            use $doc::{Common, Constraint, Drawing, EntityKind, Handle, Space};

            pub struct Sk {
                pub d: Drawing,
                pub handles: Vec<Handle>,
            }

            pub fn build(ents: &[Value], cons: &[Value]) -> Sk {
                let mut d = Drawing::default();
                let mut handles = Vec::new();
                for e in ents {
                    let kind: EntityKind = serde_json::from_value(e.clone()).expect("entity json");
                    handles.push(d.add(&Space::Model, Common::default(), kind).expect("add"));
                }
                for c in cons {
                    let c: Constraint = serde_json::from_value(c.clone()).expect("constraint json");
                    d.constraints.push(c);
                }
                Sk { d, handles }
            }

            pub fn handle_ids(sk: &Sk) -> Vec<u64> {
                sk.handles.iter().map(|h| h.0).collect()
            }

            /// The timed operation: clone (Arc bumps) and solve.
            pub fn run_sum(sk: &Sk) -> Sum {
                let mut d = sk.d.clone();
                match $con::solve(&mut d, &$con::SolveOptions::default()) {
                    Ok(r) => Sum { ok: true, moved: r.moved.len(), skipped: r.skipped.len() },
                    Err(_) => Sum { ok: false, moved: 0, skipped: 0 },
                }
            }

            /// Only the clone, to size the harness overhead inside the timed region.
            pub fn clone_only(sk: &Sk) -> usize {
                let d = sk.d.clone();
                d.constraints.len()
            }

            fn max_res(d: &Drawing) -> f64 {
                $con::residuals(d).into_iter().filter_map(|(_, r)| r).fold(0.0, |m: f64, r| m.max(r.abs()))
            }

            fn dof_text(d: &Drawing, handles: &[Handle]) -> String {
                match $con::dof(d, handles) {
                    Ok(r) => format!("dof={:?} status={:?} redundant={} constraints={}", r.dof, r.status, r.redundant, r.constraints),
                    Err(e) => format!("dof-error: {e}"),
                }
            }

            fn flatten(v: &Value, out: &mut Vec<f64>) {
                match v {
                    Value::Number(n) => out.push(n.as_f64().unwrap_or(f64::NAN)),
                    Value::Array(a) => a.iter().for_each(|x| flatten(x, out)),
                    Value::Object(m) => m.values().for_each(|x| flatten(x, out)),
                    _ => {}
                }
            }

            pub fn flat(sk: &Sk) -> Flat {
                let mut f = Flat::default();
                f.res_pre = max_res(&sk.d);
                f.dof_pre = dof_text(&sk.d, &sk.handles);
                let mut d = sk.d.clone();
                let t = Instant::now();
                let r = $con::solve(&mut d, &$con::SolveOptions::default());
                f.solve_ms = t.elapsed().as_secs_f64() * 1e3;
                match r {
                    Ok(rep) => {
                        f.result = "ok".into();
                        f.skipped = rep.skipped.len();
                    }
                    Err(e) => {
                        let s = e.to_string();
                        // Keep the variant, drop the id lists, so equal failures compare equal.
                        f.result = format!("err: {}", s.split(':').next().unwrap_or(""));
                    }
                }
                for e in d.model.iter() {
                    flatten(&serde_json::to_value(&e.kind).expect("kind json"), &mut f.params);
                }
                f.res_post = max_res(&d);
                f.satisfied = $con::all_satisfied(&d);
                f.dof_post = dof_text(&d, &sk.handles);
                f
            }
        }
    };
}

side!(old, old_constraints, old_doc);
side!(oldx, oldx_constraints, oldx_doc);
side!(new, new_constraints, new_doc);

pub struct Sketch {
    pub name: String,
    pub group: String,
    pub ents: Vec<Value>,
    pub cons: Vec<Value>,
}

enum Arm {
    Main,
    Uncapped,
}

struct Both {
    old_main: old::Sk,
    old_x: oldx::Sk,
    new: new::Sk,
    arm_is_x: bool,
    /// Solves per timed closure call, so that sub-100 us work is above timer resolution.
    batch: usize,
}

fn compare(o: &Flat, n: &Flat) -> Value {
    let both_ok = o.result == "ok" && n.result == "ok";
    let (maxdiff, len_equal) = if both_ok && o.params.len() == n.params.len() {
        (o.params.iter().zip(&n.params).map(|(a, b)| (a - b).abs()).fold(0.0f64, f64::max), true)
    } else {
        (f64::NAN, o.params.len() == n.params.len())
    };
    let verdict = if both_ok && len_equal && maxdiff <= 1e-6 {
        "agree"
    } else if both_ok {
        "DIFFER"
    } else if o.result == n.result {
        "both-fail-same"
    } else {
        "one-sided-failure"
    };
    json!({
        "verdict": verdict,
        "max_param_diff": if maxdiff.is_finite() { json!(maxdiff) } else { Value::Null },
        "old_result": o.result, "new_result": n.result,
        "old_skipped": o.skipped, "new_skipped": n.skipped,
        "old_residual_post": o.res_post, "new_residual_post": n.res_post,
        "old_residual_pre": o.res_pre, "new_residual_pre": n.res_pre,
        "old_satisfied": o.satisfied, "new_satisfied": n.satisfied,
        "dof_pre_equal": o.dof_pre == n.dof_pre, "dof_post_equal": o.dof_post == n.dof_post,
        "old_dof_pre": o.dof_pre, "new_dof_pre": n.dof_pre,
        "old_dof_post": o.dof_post, "new_dof_post": n.dof_post,
        "old_solve_ms_single": o.solve_ms, "new_solve_ms_single": n.solve_ms,
    })
}

fn load_per_core() -> (f64, usize) {
    let out = std::process::Command::new("sysctl").args(["-n", "vm.loadavg"]).output().ok();
    let s = out.map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    let l: f64 = s.split_whitespace().find_map(|t| t.trim_matches(|c| c == '{' || c == '}').parse().ok()).unwrap_or(f64::NAN);
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    (l, cores)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("run") {
        eprintln!("usage: solver-bench run --arm main|uncapped [--corpus DIR] [--out DIR] [--reps N] [--only S] [--skip-time]");
        std::process::exit(2);
    }
    let mut arm = Arm::Main;
    let mut corpus = PathBuf::from("/Users/emm/ArtCraft/corpus/cad-ezdxf");
    let mut out = PathBuf::from("../../docs/verification/data");
    let mut reps = 10usize;
    let mut only: Option<String> = None;
    let mut skip_time = false;
    let mut revs = Revs { old: None, new: None };
    let mut i = 1;
    while i < args.len() {
        let a = args[i].as_str();
        let mut val = || {
            i += 1;
            args.get(i).cloned().expect("missing value")
        };
        match a {
            "--arm" => arm = if val() == "uncapped" { Arm::Uncapped } else { Arm::Main },
            "--corpus" => corpus = PathBuf::from(val()),
            "--out" => out = PathBuf::from(val()),
            "--reps" => reps = val().parse().expect("reps"),
            "--only" => only = Some(val()),
            "--old-rev" => revs.old = Some(val()),
            "--new-rev" => revs.new = Some(val()),
            "--skip-time" => skip_time = true,
            other => panic!("unknown arg {other}"),
        }
        i += 1;
    }
    let arm_name = if matches!(arm, Arm::Main) { "main" } else { "uncapped" };
    std::fs::create_dir_all(&out).expect("out dir");

    let mut sketches = synth::synthetic();
    // Auto-constraining the corpus takes minutes; the derived sketches (corpus-derived, so never
    // committed) are cached under .cache/.
    let cache = PathBuf::from(".cache/real-sketches.json");
    let real: Vec<Sketch> = match std::fs::read_to_string(&cache).ok().and_then(|t| serde_json::from_str::<Vec<Value>>(&t).ok()) {
        Some(v) => v
            .into_iter()
            .map(|r| Sketch {
                name: r["name"].as_str().unwrap_or("").into(),
                group: "real".into(),
                ents: r["ents"].as_array().cloned().unwrap_or_default(),
                cons: r["cons"].as_array().cloned().unwrap_or_default(),
            })
            .collect(),
        None => {
            let r = synth::real(&corpus, Path::new("../../examples/apartment.dxf"));
            std::fs::create_dir_all(".cache").expect("cache dir");
            let v: Vec<Value> = r.iter().map(|s| json!({"name": s.name, "ents": s.ents, "cons": s.cons})).collect();
            std::fs::write(&cache, serde_json::to_string(&v).expect("json")).expect("cache write");
            r
        }
    };
    sketches.extend(real);
    if let Some(o) = &only {
        sketches.retain(|s| s.name.contains(o.as_str()) || s.group == *o);
    }
    eprintln!("{} sketches", sketches.len());

    // Pass 1: agreement, outside any timing.
    let mut rows: Vec<Value> = Vec::new();
    let mut eligible: Vec<(Sketch, Both, f64, f64)> = Vec::new();
    for s in sketches {
        let n = new::build(&s.ents, &s.cons);
        let (on, ox) = (old::build(&s.ents, &s.cons), oldx::build(&s.ents, &s.cons));
        assert_eq!(old::handle_ids(&on), new::handle_ids(&n), "handles differ for {}", s.name);
        assert_eq!(oldx::handle_ids(&ox), new::handle_ids(&n));
        let fnew = new::flat(&n);
        let fold = if matches!(arm, Arm::Main) { old::flat(&on) } else { oldx::flat(&ox) };
        let mut row = compare(&fold, &fnew);
        row["name"] = json!(s.name);
        row["group"] = json!(s.group);
        row["entities"] = json!(s.ents.len());
        row["constraints"] = json!(s.cons.len());
        let t = Instant::now();
        for _ in 0..200 {
            black_box(new::clone_only(black_box(&n)));
        }
        row["clone_us"] = json!(t.elapsed().as_secs_f64() * 1e6 / 200.0);
        eprintln!(
            "{:<34} {:>5} cons  {:<18} maxdiff={:<10} old {:>9.3} ms new {:>8.3} ms  dofpre= {} dofpost= {}",
            s.name,
            s.cons.len(),
            row["verdict"].as_str().unwrap_or(""),
            row["max_param_diff"].as_f64().map_or("-".into(), |v| format!("{v:.2e}")),
            fold.solve_ms,
            fnew.solve_ms,
            row["dof_pre_equal"],
            row["dof_post_equal"]
        );
        let floor_ms = fold.solve_ms.min(fnew.solve_ms).max(1e-4);
        let batch = ((0.1 / floor_ms).ceil() as usize).clamp(1, 20_000);
        row["batch"] = json!(batch);
        if row["verdict"] == "agree" {
            eligible.push((s, Both { old_main: on, old_x: ox, new: n, arm_is_x: !matches!(arm, Arm::Main), batch }, fold.solve_ms, fnew.solve_ms));
        }
        rows.push(row);
    }
    std::fs::write(out.join(format!("agreement-{arm_name}.json")), serde_json::to_string_pretty(&rows).expect("json")).expect("write");
    let n_agree = rows.iter().filter(|r| r["verdict"] == "agree").count();
    eprintln!("agreement[{arm_name}]: {n_agree}/{} agree", rows.len());
    if skip_time {
        return;
    }

    // Pass 2: paired timing, grouped by cost so warm-up and run time stay sane.
    let mut buckets: BTreeMap<String, Vec<(Sketch, Both, f64, f64)>> = BTreeMap::new();
    for e in eligible {
        let b = if e.0.group == "real" { "real".to_string() } else { format!("{}-{}", e.0.group, if e.2 > 1000.0 { "slow" } else { "fast" }) };
        buckets.entry(b).or_default().push(e);
    }
    for (bucket, items) in buckets {
        let (l, c) = load_per_core();
        eprintln!("bucket {bucket}: {} inputs, load {l:.2} on {c} cores (limit {:.1})", items.len(), 0.5 * c as f64);
        let slow = items.iter().any(|i| i.2 > 1000.0);
        let cfg = Config { repetitions: reps, warmup: if slow { 1 } else { 2 }, revs: revs.clone(), ..Config::default() };
        let name = format!("cadcraft solver {arm_name}->new [{bucket}]");
        let report = Paired::new(&name)
            .old(|b: &Both| {
                let mut last = Sum { ok: false, moved: 0, skipped: 0 };
                for _ in 0..b.batch {
                    last = if b.arm_is_x { oldx::run_sum(&b.old_x) } else { old::run_sum(&b.old_main) };
                }
                last
            })
            .new(|b: &Both| {
                let mut last = Sum { ok: false, moved: 0, skipped: 0 };
                for _ in 0..b.batch {
                    last = new::run_sum(black_box(&b.new));
                }
                last
            })
            .inputs(items.into_iter().map(|(s, b, _, _)| (s.name, b)))
            .check(|a: &Sum, b: &Sum| a.ok == b.ok)
            .run(&cfg);
        match report {
            Ok(r) => {
                std::fs::write(out.join(format!("solver-{arm_name}-{bucket}.json")), r.to_json()).expect("write");
                println!("{}", r.to_markdown());
                if let Err(e) = r.require_publishable() {
                    eprintln!("NOT PUBLISHABLE ({bucket}): {e}");
                }
            }
            Err(e) => eprintln!("bench {bucket} failed: {e}"),
        }
    }
}
