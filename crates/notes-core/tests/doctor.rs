use notes_core::Library;
use std::{fs, os::unix::fs::PermissionsExt};

fn setup() -> (tempfile::TempDir, Library) {
    let t = tempfile::tempdir().unwrap();
    fs::create_dir(t.path().join("notes")).unwrap();
    let lib = Library::open(&t.path().join("notes"), &t.path().join("state"), false).unwrap();
    fs::write(lib.root().join("a.md"), "# Original\n").unwrap();
    (t, lib)
}
fn has(r: &notes_core::index::DoctorReport, kind: &str) -> bool {
    r.diagnostics.iter().any(|d| d.kind == kind)
}
#[test]
fn doctor_missing_cache_does_not_create_state() {
    let (t, lib) = setup();
    let r = lib.doctor().unwrap();
    assert!(has(&r, "cache_missing"));
    assert_eq!(r.discovered_notes, 1);
    assert!(!t.path().join("state").exists());
    assert_eq!(fs::read(lib.root().join("a.md")).unwrap(), b"# Original\n");
}
#[test]
fn doctor_finds_stale_orphan_missing_and_yaml_without_reconciling() {
    let (_t, lib) = setup();
    fs::write(lib.root().join("gone.md"), "old").unwrap();
    lib.reindex().unwrap();
    let before = fs::read(lib.cache_path().unwrap()).unwrap();
    fs::write(lib.root().join("a.md"), "changed").unwrap();
    fs::remove_file(lib.root().join("gone.md")).unwrap();
    fs::write(lib.root().join("new.md"), "new").unwrap();
    fs::write(lib.root().join("bad.md"), "---\ntags: [\n---\nbody").unwrap();
    let r = lib.doctor().unwrap();
    for kind in [
        "stale_record",
        "orphan_record",
        "unindexed_note",
        "source_metadata",
    ] {
        assert!(has(&r, kind), "missing {kind}: {r:?}");
    }
    assert_eq!(fs::read(lib.cache_path().unwrap()).unwrap(), before);
    assert_eq!(
        fs::read_to_string(lib.root().join("a.md")).unwrap(),
        "changed"
    );
}
#[test]
fn doctor_checks_schema_corruption_and_explicit_existing_repair() {
    let (_t, lib) = setup();
    lib.reindex().unwrap();
    assert!(!lib.doctor().unwrap().incomplete);
    let path = lib.cache_path().unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("PRAGMA user_version=99").unwrap();
    drop(conn);
    let before = fs::read(&path).unwrap();
    assert!(has(&lib.doctor().unwrap(), "cache_schema"));
    assert_eq!(fs::read(&path).unwrap(), before);
    fs::write(&path, b"not sqlite").unwrap();
    assert!(has(&lib.doctor().unwrap(), "cache_corrupt"));
    assert_eq!(fs::read(&path).unwrap(), b"not sqlite");
    lib.reindex().unwrap();
    assert!(!lib.doctor().unwrap().incomplete);
    assert_eq!(fs::read(lib.root().join("a.md")).unwrap(), b"# Original\n");
}
#[test]
fn doctor_unreadable_subtree_is_unknown_not_orphan() {
    let (_t, lib) = setup();
    let dir = lib.root().join("private");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("b.md"), "secret test fixture").unwrap();
    lib.reindex().unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o000)).unwrap();
    let r = lib.doctor().unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(has(&r, "source_access"), "{r:?}");
    assert!(has(&r, "unverified_record"));
    assert!(!has(&r, "orphan_record"));
}
#[test]
fn doctor_detects_derived_table_drift_without_repairing_it() {
    let (_t, lib) = setup();
    lib.reindex().unwrap();
    let path = lib.cache_path().unwrap();
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute("DELETE FROM notes_fts", []).unwrap();
    drop(conn);
    let before = fs::read(&path).unwrap();
    assert!(has(&lib.doctor().unwrap(), "cache_inconsistent"));
    assert_eq!(fs::read(&path).unwrap(), before);
    lib.reindex().unwrap();
    assert!(!lib.doctor().unwrap().incomplete);
}

#[test]
fn doctor_checks_escaped_cache_uri_and_refuses_unsafe_cache_files() {
    let temp = tempfile::tempdir().unwrap();
    let state = temp.path().join("state?#% spaces");
    let lib = Library::open(&temp.path().join("notes"), &state, true).unwrap();
    fs::write(lib.root().join("a.md"), "# A\n").unwrap();
    lib.reindex().unwrap();
    assert!(!lib.doctor().unwrap().incomplete);
    let cache = lib.cache_path().unwrap();
    let original = fs::read(&cache).unwrap();
    let copy = temp.path().join("outside.db");
    fs::rename(&cache, &copy).unwrap();
    std::os::unix::fs::symlink(&copy, &cache).unwrap();
    let report = lib.doctor().unwrap();
    assert!(has(&report, "cache_access"));
    assert!(!report.reindex_safe);
    assert_eq!(fs::read(&copy).unwrap(), original);
    fs::remove_file(&cache).unwrap();
    fs::hard_link(&copy, &cache).unwrap();
    assert!(has(&lib.doctor().unwrap(), "cache_access"));
    assert_eq!(fs::read(&copy).unwrap(), original);
}

#[test]
fn doctor_refuses_sidecars_without_modifying_them() {
    let (_t, lib) = setup();
    lib.reindex().unwrap();
    let sidecar = format!("{}-wal", lib.cache_path().unwrap().display());
    fs::write(&sidecar, "pending").unwrap();
    assert!(has(&lib.doctor().unwrap(), "cache_busy"));
    assert_eq!(fs::read_to_string(sidecar).unwrap(), "pending");
}
