#[allow(dead_code)]
#[path = "../src/config.rs"]
mod config;
#[allow(dead_code)]
#[path = "../src/parity.rs"]
mod parity;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use craft_xtask::{ico, layers, stats, version};
use serde_json::{Value, json};

static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

struct Temp(PathBuf);

impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("cadcraft-70-{}-{}", std::process::id(), NEXT_TEMP.fetch_add(1, Ordering::Relaxed)));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) {
        let path = self.0.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn metadata() -> Value {
    serde_json::from_str(include_str!("golden/metadata.json")).unwrap()
}

#[test]
fn layers_report_matches_main_byte_for_byte() {
    let (report, result) = config::layer_report(&config::layers(Path::new(".")), &metadata()).unwrap();
    result.unwrap();
    assert_eq!(report.as_bytes(), include_bytes!("golden/layers.stdout"));
    let output = Command::new(env!("CARGO_BIN_EXE_xtask")).arg("layers").output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(output.stdout, include_bytes!("golden/layers.stdout"));
}

#[test]
fn layer_policy_rejects_forbidden_edges() {
    let policy = config::layers(Path::new("."));
    let graph = layers::from_metadata(&metadata()).unwrap();
    for (source, target, workspace, kind) in [
        ("cadcraft-geom", "cadcraft-doc", true, layers::DepKind::Normal),
        ("cadcraft-dxf", "cadcraft-geom", true, layers::DepKind::Dev),
        ("cadcraft-engine", "cadcraft-testkit", true, layers::DepKind::Normal),
        ("cadcraft-render", "egui", false, layers::DepKind::Build),
        ("cadcraft-geom", "cadcraft-color", true, layers::DepKind::Normal),
        ("cadcraft-fonts", "cadcraft-render", true, layers::DepKind::Normal),
    ] {
        let mut mutated = graph.clone();
        mutated.iter_mut().find(|krate| krate.name == source).unwrap().deps.push(layers::Dep { name: target.into(), kind, workspace });
        assert_eq!(layers::check(&policy, &mutated).unwrap().len(), 1, "{source} -> {target}");
    }
}

#[test]
fn layer_policy_preserves_allowed_edges_and_wasm_selection() {
    let policy = config::layers(Path::new("."));
    let mut graph = layers::from_metadata(&metadata()).unwrap();
    graph.iter_mut().find(|krate| krate.name == "cadcraft-geom").unwrap().deps.push(layers::Dep {
        name: "cadcraft-testkit".into(),
        kind: layers::DepKind::Dev,
        workspace: true,
    });
    assert!(layers::check(&policy, &graph).unwrap().is_empty());
    let selected: Vec<_> = graph.iter().filter(|krate| config::wasm_safe(&policy, &krate.name)).map(|krate| krate.name.as_str()).collect();
    assert_eq!(
        selected,
        [
            "cadcraft-color",
            "cadcraft-constraints",
            "cadcraft-doc",
            "cadcraft-dwg",
            "cadcraft-dxf",
            "cadcraft-engine",
            "cadcraft-fonts",
            "cadcraft-geom",
            "cadcraft-io",
            "cadcraft-render",
            "cadcraft-ui-egui"
        ]
    );
}

#[test]
fn version_read_matches_main() {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask")).arg("version").output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, include_bytes!("golden/version.stdout"));
}

#[test]
fn version_edits_real_manifests_and_lockfile_identically() {
    if let Some(root) = std::env::var_os("CADCRAFT_GOLDEN_VERSION_ROOT") {
        let result = version::run(&config::version(Path::new(&root)), &["set".into(), "1.2.3-rc.1".into()]);
        if let Err(error) = &result {
            eprintln!("{error}");
        }
        std::process::exit(if result.is_ok() { 0 } else { 1 });
    }
    let temp = Temp::new();
    let inputs: Value = serde_json::from_str(include_str!("golden/version-inputs.json")).unwrap();
    for (path, text) in inputs["manifests"].as_object().unwrap() {
        temp.write(path, text.as_str().unwrap());
    }
    for target in inputs["targets"].as_array().unwrap() {
        temp.write(target.as_str().unwrap(), "");
    }
    temp.write("Cargo.lock", include_bytes!("golden/version-before.lock"));
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "version_edits_real_manifests_and_lockfile_identically", "--nocapture", "--quiet", "--test-threads=1"])
        .env("CADCRAFT_GOLDEN_VERSION_ROOT", &temp.0)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let stdout = output.stdout.strip_prefix(b"\nrunning 1 test\n").unwrap();
    assert_eq!(stdout, include_bytes!("golden/version-set.stdout"));
    assert_eq!(std::fs::read(temp.0.join("Cargo.toml")).unwrap(), include_bytes!("golden/version-after.toml"));
    assert_eq!(std::fs::read(temp.0.join("Cargo.lock")).unwrap(), include_bytes!("golden/version-after.lock"));
    for (path, text) in inputs["manifests"].as_object().unwrap().iter().filter(|(path, _)| path.as_str() != "Cargo.toml") {
        assert_eq!(std::fs::read_to_string(temp.0.join(path)).unwrap(), text.as_str().unwrap());
    }
}

#[test]
fn version_keeps_permissive_validation_and_line_endings() {
    let policy = config::version(Path::new("."));
    assert!(!policy.strict_prerelease && !policy.bounded_core && !policy.rollback_on_failure && !policy.update_path_pins);
    assert!(policy.refresh_lockfile && policy.online_fallback);
    let input = "[workspace.package]\r\nversion = \"0.17.0\"\r\n";
    assert_eq!(version::rewrite(&policy, input, "1.2.3-01").unwrap(), "[workspace.package]\r\nversion = \"1.2.3-01\"\r\n");
    assert!(version::validate("01.2.3").is_err());
}

#[test]
fn stats_replays_original_tree_counts_byte_for_byte() {
    let temp = Temp::new();
    let counts: Value = serde_json::from_str(include_str!("golden/stats-counts.json")).unwrap();
    let mut packages = Vec::new();
    for row in counts.as_array().unwrap() {
        let name = row["name"].as_str().unwrap();
        let files = row["files"].as_u64().unwrap() as usize;
        let lines = row["lines"].as_u64().unwrap() as usize;
        let tests = row["tests"].as_u64().unwrap() as usize;
        let props = row["props"].as_u64().unwrap() as usize;
        let mut source = "#[test]\n".repeat(tests);
        source += &"proptest! {}\n".repeat(props);
        source += &"\n".repeat(lines - tests - props);
        temp.write(format!("{name}/src/0.rs"), source);
        for index in 1..files {
            temp.write(format!("{name}/src/{index}.rs"), "");
        }
        packages.push(json!({"name": name, "manifest_path": format!("{name}/Cargo.toml")}));
    }
    let report = stats::report(&config::stats(&temp.0), &json!({"packages": packages}), false).unwrap();
    assert_eq!(report.as_bytes(), include_bytes!("golden/stats.stdout"));
}

#[test]
fn stats_preserves_exclusions_and_budget() {
    let temp = Temp::new();
    temp.write("src/lib.rs", "// #[test]\n#[test] fn works() {}\nproptest! {}\n");
    for excluded in ["target", "fuzz", ".hidden"] {
        temp.write(format!("{excluded}/ignored.rs"), "#[test]\n");
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(temp.0.join("src"), temp.0.join("alias")).unwrap();
    let mut policy = config::stats(&temp.0);
    assert_eq!(stats::count(&policy, &temp.0).unwrap(), stats::Counts { files: 1, lines: 3, tests: 1, proptests: 1 });
    policy.max_entries = 0;
    assert!(stats::count(&policy, &temp.0).is_err());
}

#[test]
fn ico_output_and_bytes_match_main() {
    let temp = Temp::new();
    let images = [
        (16, include_bytes!("golden/icon-16.bin")),
        (32, include_bytes!("golden/icon-32.bin")),
        (48, include_bytes!("golden/icon-48.bin")),
        (128, include_bytes!("golden/icon-128.bin")),
        (256, include_bytes!("golden/icon-256.bin")),
    ];
    let mut command = Command::new(env!("CARGO_BIN_EXE_xtask"));
    command.current_dir(&temp.0).args(["ico", "out.ico"]);
    for (size, bytes) in images {
        let path = format!("{size}.png");
        temp.write(&path, bytes);
        command.arg(path);
    }
    let output = command.output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(output.stdout, include_bytes!("golden/ico.stdout"));
    assert_eq!(std::fs::read(temp.0.join("out.ico")).unwrap(), include_bytes!("golden/ico.bin"));
    assert!(ico::pack(&[]).is_err());
}

#[test]
fn parity_inventory_and_report_match_main_byte_for_byte() {
    let commands: Value = serde_json::from_str(include_str!("golden/commands.json")).unwrap();
    let (markdown, summary) = parity::report(include_str!("golden/menu-catalog.txt"), &commands, "2026-11-03");
    assert_eq!(markdown.as_bytes(), include_bytes!("golden/parity.md"));
    assert_eq!(format!("{summary}\n").as_bytes(), include_bytes!("golden/parity.stdout"));
}
