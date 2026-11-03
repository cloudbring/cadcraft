use std::path::Path;

use craft_xtask::{Cargo, Result, layers, stats, version};

pub fn version(root: &Path) -> version::Config {
    version::Config { cargo: Cargo { root: root.into(), ..Cargo::default() }, ..version::Config::default() }
}

pub fn stats(root: &Path) -> stats::Config {
    stats::Config { cargo: Cargo { root: root.into(), ..Cargo::default() }, max_directories: usize::MAX, ..stats::Config::default() }
}

fn names(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).into()).collect()
}

pub fn layers(root: &Path) -> layers::Config {
    layers::Config {
        root: root.into(),
        layers: [
            ("L0", names(&["cadcraft-geom", "cadcraft-dxf", "cadcraft-dwg", "cadcraft-color"])),
            ("L1", names(&["cadcraft-doc"])),
            ("L2", names(&["cadcraft-constraints", "cadcraft-fonts", "cadcraft-render"])),
            ("L3", names(&["cadcraft-io"])),
            ("L4", names(&["cadcraft-engine"])),
            ("L5", names(&["cadcraft-ui-egui", "cadcraft-mcp"])),
            ("L6", names(&["cadcraft-testkit"])),
        ]
        .into_iter()
        .map(|(label, members)| (label.into(), members))
        .collect(),
        exempt: names(&["cadcraft", "cadcraft-cli", "cadcraft-web", "xtask"]),
        standalone: names(&["cadcraft-dxf", "cadcraft-dwg"]),
        testkits: names(&["cadcraft-testkit"]),
        allowed_edges: vec![("cadcraft-color".into(), "cadcraft-geom".into()), ("cadcraft-render".into(), "cadcraft-fonts".into())],
        external_rules: vec![layers::ExternalRule {
            dependencies: names(&["craft-ui", "egui", "eframe", "egui-wgpu", "wgpu", "winit", "egui_kittest", "rfd", "bevy*"]),
            allowed_from: names(&["cadcraft-ui-egui", "cadcraft-mcp", "cadcraft-testkit"]),
            allowed_dev_from: Vec::new(),
        }],
        ..layers::Config::default()
    }
}

pub fn wasm_safe(policy: &layers::Config, name: &str) -> bool {
    policy.layers.iter().take(5).any(|(_, members)| members.iter().any(|member| member == name)) || name == "cadcraft-ui-egui"
}

fn describe(policy: &layers::Config, name: &str) -> String {
    if policy.exempt.iter().any(|member| member == name) {
        "exempt".into()
    } else if policy.standalone.iter().any(|member| member == name) {
        "L0 standalone".into()
    } else if policy.testkits.iter().any(|member| member == name) {
        "testkit".into()
    } else {
        policy
            .layers
            .iter()
            .find(|(_, members)| members.iter().any(|member| member == name))
            .map(|(label, _)| label.clone())
            .unwrap_or("UNREGISTERED".into())
    }
}

pub fn layer_report(policy: &layers::Config, metadata: &serde_json::Value) -> Result<(String, Result)> {
    let crates = layers::from_metadata(metadata)?;
    let mut report = format!("Dependency layering (plan/architecture.md §3)\n\n{:<28} {:<14} workspace deps\n", "crate", "layer");
    for krate in &crates {
        let deps = krate
            .deps
            .iter()
            .filter(|dep| dep.workspace)
            .map(|dep| {
                let suffix = match dep.kind {
                    layers::DepKind::Normal => "",
                    layers::DepKind::Dev => " (dev)",
                    layers::DepKind::Build => " (build)",
                };
                format!("{}{suffix}", dep.name.strip_prefix("cadcraft-").unwrap_or(&dep.name))
            })
            .collect::<Vec<_>>();
        report += &format!("{:<28} {:<14} {}\n", krate.name, describe(policy, &krate.name), deps.join(", "));
    }
    let violations = layers::check(policy, &crates)?;
    report += "\n";
    let result = if violations.is_empty() {
        report += &format!("OK: {} crates, no layering violations.\n", crates.len());
        Ok(())
    } else {
        report += &format!("{} violation(s):\n", violations.len());
        for violation in &violations {
            report += &format!("  - {violation}\n");
        }
        Err(format!("{} layering violation(s)", violations.len()))
    };
    Ok((report, result))
}
