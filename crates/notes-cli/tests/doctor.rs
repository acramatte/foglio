use std::{fs, path::Path, process::Command};

fn run(base: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_notes"))
        .args([
            "--library",
            base.join("library").to_str().unwrap(),
            "--json",
        ])
        .args(args)
        .env_clear()
        .env("HOME", base.join("home"))
        .env("XDG_CONFIG_HOME", base.join("config"))
        .env("XDG_CACHE_HOME", base.join("cache"))
        .output()
        .unwrap()
}
fn snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        for item in fs::read_dir(dir).unwrap() {
            let p = item.unwrap().path();
            out.push((
                p.strip_prefix(root).unwrap().to_str().unwrap().into(),
                if p.is_file() {
                    fs::read(&p).unwrap()
                } else {
                    Vec::new()
                },
            ));
            if p.is_dir() {
                walk(root, &p, out);
            }
        }
    }
    let mut result = Vec::new();
    walk(root, root, &mut result);
    result.sort();
    result
}
#[test]
fn doctor_is_read_only_and_reindex_is_the_explicit_repair() {
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path();
    fs::create_dir(base.join("library")).unwrap();
    fs::write(base.join("library/a.md"), "# Ordinary\n").unwrap();
    let before = snapshot(base);
    let output = run(base, &["doctor"]);
    assert_eq!(output.status.code(), Some(5), "{output:?}");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["diagnostics"][0]["kind"], "cache_missing");
    assert_eq!(snapshot(base), before);
    assert!(run(base, &["reindex"]).status.success());
    let before = snapshot(base);
    assert!(run(base, &["doctor"]).status.success());
    assert_eq!(snapshot(base), before);
    fs::write(base.join("library/a.md"), "# Changed\n").unwrap();
    let before = snapshot(base);
    let output = run(base, &["doctor"]);
    assert_eq!(output.status.code(), Some(5));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["result"]["stale_notes"], 1);
    assert!(
        value["diagnostics"][0]["action"]
            .as_str()
            .unwrap()
            .contains("reindex")
    );
    assert_eq!(snapshot(base), before);
    assert!(run(base, &["reindex"]).status.success());
    assert!(run(base, &["doctor"]).status.success());
    assert_eq!(fs::read(base.join("library/a.md")).unwrap(), b"# Changed\n");
}
