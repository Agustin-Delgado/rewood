//! Regression fixtures: every directory under `fixtures/` holds an
//! `input.json` and the `expected.json` plan it must compile to, byte for
//! byte after canonical rounding. If the engine moves a hole 0.001 mm, this
//! fails.
//!
//! Regenerate on purpose with `UPDATE_FIXTURES=1 cargo test -p rewood-core`.

use std::path::{Path, PathBuf};

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// First path where two JSON values differ, for a readable failure.
fn first_difference(a: &serde_json::Value, b: &serde_json::Value, path: &str) -> Option<String> {
    use serde_json::Value;
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for key in x.keys().chain(y.keys().filter(|k| !x.contains_key(*k))) {
                let p = format!("{path}.{key}");
                match (x.get(key), y.get(key)) {
                    (Some(va), Some(vb)) => {
                        if let Some(d) = first_difference(va, vb, &p) {
                            return Some(d);
                        }
                    }
                    (Some(_), None) => return Some(format!("{p}: falta en expected")),
                    (None, Some(_)) => return Some(format!("{p}: falta en actual")),
                    (None, None) => unreachable!(),
                }
            }
            None
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                return Some(format!(
                    "{path}: {} elementos vs {} en expected",
                    x.len(),
                    y.len()
                ));
            }
            x.iter()
                .zip(y)
                .enumerate()
                .find_map(|(i, (va, vb))| first_difference(va, vb, &format!("{path}[{i}]")))
        }
        _ if a == b => None,
        _ => Some(format!("{path}: {a} vs {b} en expected")),
    }
}

#[test]
fn fixtures_compile_to_expected_plans() {
    let update = std::env::var("UPDATE_FIXTURES").is_ok();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(fixtures_dir())
        .expect("fixtures dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.join("input.json").exists())
        .collect();
    dirs.sort();
    assert!(!dirs.is_empty(), "no fixtures found");

    let mut failures = Vec::new();
    for dir in dirs {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let input = std::fs::read_to_string(dir.join("input.json")).unwrap();
        let actual = rewood_core::compile_json(&input).to_json_value();
        let expected_path = dir.join("expected.json");
        if update || !expected_path.exists() {
            std::fs::write(
                &expected_path,
                serde_json::to_string_pretty(&actual).unwrap() + "\n",
            )
            .unwrap();
            eprintln!("fixture {name}: expected.json escrito");
            continue;
        }
        let expected: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&expected_path).unwrap()).unwrap();
        if let Some(diff) = first_difference(&actual, &expected, "plan") {
            failures.push(format!("{name}: {diff}"));
        }
    }
    assert!(
        failures.is_empty(),
        "fixtures con diferencias:\n{}",
        failures.join("\n")
    );
}
