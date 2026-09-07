use notes_core::{Library, search::SearchQuery};
use std::fs;

fn setup() -> (tempfile::TempDir, Library) {
    let temp = tempfile::tempdir().unwrap();
    let lib = Library::open(&temp.path().join("notes"), &temp.path().join("state"), true).unwrap();
    (temp, lib)
}

#[test]
fn index_lifecycle_warm_reuse_and_disposable_cache() {
    let (_temp, lib) = setup();
    let entry = lib
        .create("a.md", "# Alpha\nneedle", &["Work".into()])
        .unwrap();
    let bytes = fs::read(lib.root().join("a.md")).unwrap();
    let warm = lib.status().unwrap();
    assert_eq!(warm.indexed_notes, 1);
    assert_eq!(warm.parsed_notes, 0);
    assert!(!warm.incomplete);
    assert!(!warm.watcher_active);
    assert_eq!(
        lib.search(&SearchQuery::literal("needle"))
            .unwrap()
            .hits
            .len(),
        1
    );
    let commit = lib
        .update("a.md", &entry.document.revision, "# Beta\nnewword")
        .unwrap();
    assert!(commit.file_committed);
    assert_eq!(
        lib.search(&SearchQuery::literal("needle"))
            .unwrap()
            .hits
            .len(),
        0
    );
    lib.move_note("a.md", commit.revision.as_ref().unwrap(), "folder/b.md")
        .unwrap();
    assert_eq!(
        lib.search(&SearchQuery::literal("newword")).unwrap().hits[0].path,
        "folder/b.md"
    );
    let before = fs::read(lib.root().join("folder/b.md")).unwrap();
    assert_ne!(before, bytes);
    fs::remove_file(lib.cache_path().unwrap()).unwrap();
    assert_eq!(lib.reindex().unwrap().indexed_notes, 1);
    fs::write(lib.cache_path().unwrap(), b"not sqlite").unwrap();
    assert!(lib.reindex().unwrap().cache_rebuilt);
    assert_eq!(fs::read(lib.root().join("folder/b.md")).unwrap(), before);
    let e = lib.get("folder/b.md").unwrap();
    lib.delete("folder/b.md", &e.document.revision).unwrap();
    assert_eq!(lib.status().unwrap().indexed_notes, 0);
}

#[test]
fn duplicate_groups_and_malformed_identity_are_not_searchable() {
    let (_temp, lib) = setup();
    lib.create("a.md", "# Unique\nneedle", &[]).unwrap();
    fs::copy(lib.root().join("a.md"), lib.root().join("b.md")).unwrap();
    let status = lib.rescan().unwrap();
    assert_eq!(status.ambiguous_notes, 2);
    assert_eq!(status.indexed_notes, 0);
    assert!(status.incomplete);
    fs::remove_file(lib.root().join("b.md")).unwrap();
    assert_eq!(lib.rescan().unwrap().indexed_notes, 1);
    let source = fs::read_to_string(lib.root().join("a.md"))
        .unwrap()
        .replace("tags: []", "tags: 12");
    fs::write(lib.root().join("bad.md"), source).unwrap();
    let status = lib.rescan().unwrap();
    assert!(status.incomplete);
    assert_eq!(
        lib.search(&SearchQuery::literal("needle"))
            .unwrap()
            .hits
            .len(),
        0
    );
}

#[test]
fn same_size_offline_edits_and_path_dependent_titles_are_reconciled() {
    use std::fs::FileTimes;
    let (_temp, lib) = setup();
    lib.create("old.md", "apple", &[]).unwrap();
    let p = lib.root().join("old.md");
    let times = fs::metadata(&p).unwrap();
    let source = fs::read_to_string(&p).unwrap().replace("apple", "peach");
    fs::write(&p, source).unwrap();
    fs::File::options()
        .write(true)
        .open(&p)
        .unwrap()
        .set_times(FileTimes::new().set_modified(times.modified().unwrap()))
        .unwrap();
    assert_eq!(lib.rescan().unwrap().parsed_notes, 1);
    assert_eq!(
        lib.search(&SearchQuery::literal("peach"))
            .unwrap()
            .hits
            .len(),
        1
    );
    fs::rename(&p, lib.root().join("renamed.md")).unwrap();
    assert_eq!(lib.reindex().unwrap().indexed_notes, 1);
    assert_eq!(
        lib.search(&SearchQuery::literal("peach")).unwrap().hits[0].title,
        "renamed"
    );
}

#[test]
fn cache_symlinks_hardlinks_sidecars_and_future_schema_are_refused() {
    use std::os::unix::fs::symlink;
    let (temp, lib) = setup();
    lib.create("a.md", "body", &[]).unwrap();
    let cache = lib.cache_path().unwrap();
    let outside = temp.path().join("outside");
    fs::write(&outside, "untouched").unwrap();
    fs::remove_file(&cache).unwrap();
    symlink(&outside, &cache).unwrap();
    assert!(lib.reindex().is_err());
    fs::remove_file(&cache).unwrap();
    fs::hard_link(&outside, &cache).unwrap();
    assert!(lib.reindex().is_err());
    fs::remove_file(&cache).unwrap();
    lib.reindex().unwrap();
    let sidecar = format!("{}-wal", cache.display());
    symlink(&outside, &sidecar).unwrap();
    assert!(lib.reindex().is_err());
    fs::remove_file(sidecar).unwrap();
    let conn = rusqlite::Connection::open(&cache).unwrap();
    conn.execute_batch("PRAGMA user_version=999").unwrap();
    drop(conn);
    assert!(lib.reindex().is_err());
    assert_eq!(fs::read_to_string(outside).unwrap(), "untouched");
    assert!(lib.root().join("a.md").exists());
}

#[test]
fn warm_status_recovers_corrupt_fts_without_modifying_notes() {
    let (_temp, lib) = setup();
    lib.create("a.md", "# Searchable\nneedle", &[]).unwrap();
    let before = fs::read(lib.root().join("a.md")).unwrap();
    let conn = rusqlite::Connection::open(lib.cache_path().unwrap()).unwrap();
    conn.execute("UPDATE notes_fts_data SET block=x'00' WHERE id>10", [])
        .unwrap();
    drop(conn);
    let status = lib.status().unwrap();
    assert!(status.cache_rebuilt);
    assert_eq!(status.indexed_notes, 1);
    assert!(!status.incomplete);
    assert_eq!(
        lib.search(&SearchQuery::literal("needle"))
            .unwrap()
            .hits
            .len(),
        1
    );
    assert_eq!(fs::read(lib.root().join("a.md")).unwrap(), before);
}

#[test]
fn search_snapshot_blocks_rebuild_until_connection_is_closed() {
    let (_temp, lib) = setup();
    lib.create("a.md", "body", &[]).unwrap();
    let session = lib.search_session().unwrap();
    assert_eq!(lib.reindex().unwrap_err().code, notes_core::ErrorCode::Busy);
    assert_eq!(
        session.search(&SearchQuery::literal("body")).unwrap().len(),
        1
    );
    drop(session);
    assert_eq!(lib.reindex().unwrap().indexed_notes, 1);
}

#[test]
fn cache_failure_after_file_commit_is_not_an_untouched_error() {
    let (_temp, lib) = setup();
    let entry = lib.create("a.md", "old", &[]).unwrap();
    let cache = lib.cache_path().unwrap();
    fs::remove_file(&cache).unwrap();
    fs::create_dir(&cache).unwrap();
    let err = lib
        .update("a.md", &entry.document.revision, "saved on disk")
        .unwrap_err();
    assert_eq!(err.code, notes_core::ErrorCode::Committed);
    assert!(err.commit.unwrap().file_committed);
    assert!(
        fs::read_to_string(lib.root().join("a.md"))
            .unwrap()
            .contains("saved on disk")
    );
    fs::remove_dir(&cache).unwrap();
    assert_eq!(lib.reindex().unwrap().indexed_notes, 1);
}
