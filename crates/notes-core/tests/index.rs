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
fn identical_content_paths_are_searchable_and_malformed_files_are_excluded() {
    let (_temp, lib) = setup();
    let source = "---\nid: arbitrary\n---\n# Unique\nneedle";
    for path in ["a.md", "b.md"] {
        fs::write(lib.root().join(path), source).unwrap();
    }
    let status = lib.rescan().unwrap();
    assert_eq!(status.indexed_notes, 2);
    assert!(!status.incomplete);
    let hits = lib.search(&SearchQuery::literal("needle")).unwrap().hits;
    assert_eq!(
        hits.iter().map(|h| h.path.as_str()).collect::<Vec<_>>(),
        ["a.md", "b.md"]
    );
    assert_eq!(
        serde_json::to_value(&hits[0])
            .unwrap()
            .as_object()
            .unwrap()
            .len(),
        4
    );
    fs::write(lib.root().join("bad.md"), "---\ntags: 12\n---\nneedle").unwrap();
    let status = lib.rescan().unwrap();
    assert!(status.incomplete);
    assert_eq!(status.indexed_notes, 2);
    assert_eq!(
        lib.search(&SearchQuery::literal("needle"))
            .unwrap()
            .hits
            .len(),
        2
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
    conn.execute_batch("PRAGMA user_version=999; PRAGMA journal_mode=WAL;")
        .unwrap();
    drop(conn);
    let before = fs::read(&cache).unwrap();
    assert!(lib.reindex().is_err());
    assert!(
        fs::read(&cache).unwrap() == before,
        "future schema bytes changed"
    );
    let conn = rusqlite::Connection::open(&cache).unwrap();
    assert_eq!(
        conn.query_row("PRAGMA journal_mode", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "wal"
    );
    drop(conn);
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

#[test]
fn schema_one_cache_is_discarded_and_rebuilt_without_source_writes() {
    let (_temp, lib) = setup();
    let sources = [
        ("plain.md", "# Needle"),
        ("a.md", "---\nid: arbitrary\n---\n# Needle"),
        ("b.md", "---\nid: arbitrary\n---\n# Needle"),
    ];
    for (path, source) in sources {
        fs::write(lib.root().join(path), source).unwrap();
    }
    let cache = lib.cache_path().unwrap();
    fs::create_dir_all(cache.parent().unwrap()).unwrap();
    let conn = rusqlite::Connection::open(&cache).unwrap();
    conn.execute_batch(include_str!("../migrations/001_index.sql"))
        .unwrap();
    conn.execute(
        "INSERT INTO files VALUES('ghost.md','old-id','Ghost','ghost','[]','old','old',0)",
        [],
    )
    .unwrap();
    drop(conn);
    let status = lib.status().unwrap();
    assert!(status.cache_rebuilt);
    assert_eq!(status.parsed_notes, 3);
    assert_eq!(status.indexed_notes, 3);
    assert!(!status.incomplete);
    let conn = rusqlite::Connection::open(&cache).unwrap();
    assert_eq!(
        conn.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM files WHERE path='ghost.md'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    assert!(conn.prepare("SELECT id FROM notes").is_err());
    drop(conn);
    assert_eq!(
        lib.search(&SearchQuery::literal("needle"))
            .unwrap()
            .hits
            .len(),
        3
    );
    for (path, source) in sources {
        assert_eq!(fs::read(lib.root().join(path)).unwrap(), source.as_bytes());
    }
    assert!(!lib.status().unwrap().cache_rebuilt);
}

#[test]
fn failed_rebuild_rolls_back_notes_tags_files_and_fts_together() {
    let (_temp, lib) = setup();
    lib.create("a.md", "before", &["old".into()]).unwrap();
    let cache = lib.cache_path().unwrap();
    let conn = rusqlite::Connection::open(&cache).unwrap();
    conn.execute_batch("CREATE TRIGGER fail_insert BEFORE INSERT ON notes BEGIN SELECT RAISE(ABORT, 'injected'); END;").unwrap();
    fs::write(lib.root().join("a.md"), "---\ntags: [new]\n---\nafter").unwrap();
    assert_eq!(
        lib.reindex().unwrap_err().code,
        notes_core::ErrorCode::Index
    );
    assert_eq!(
        conn.query_row("SELECT body FROM files", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "before"
    );
    assert_eq!(
        conn.query_row("SELECT tag FROM tags", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "old"
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM notes", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        conn.query_row(
            "SELECT count(*) FROM notes_fts WHERE notes_fts MATCH 'before'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        1
    );
    conn.execute_batch("DROP TRIGGER fail_insert").unwrap();
    drop(conn);
    assert_eq!(lib.reindex().unwrap().indexed_notes, 1);
    assert_eq!(
        lib.search(&SearchQuery::literal("after"))
            .unwrap()
            .hits
            .len(),
        1
    );
}
