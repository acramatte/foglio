use notes_desktop::{Backend, external_url, relative_link};
use std::{
    fs,
    sync::{Arc, Barrier},
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;
fn fixture() -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("notes");
    let state = temp.path().join("state");
    fs::create_dir_all(root.join("folder/empty")).unwrap();
    fs::write(
        root.join("folder/a.md"),
        "---\ntags: [rust]\ncustom: preserved\n---\n# Alpha\nneedle\n",
    )
    .unwrap();
    (temp, root, state)
}
#[test]
fn readonly_browse_search_open_and_selection_preserve_bytes() {
    let (_temp, root, state) = fixture();
    fs::write(root.join("plain.md"), "# Import\nuntouched\n").unwrap();
    fs::write(root.join("metadata.md"), "---\nid: invalid\n---\nbad").unwrap();
    let before: Vec<_> = ["folder/a.md", "plain.md", "metadata.md"]
        .into_iter()
        .map(|p| (p, fs::read(root.join(p)).unwrap()))
        .collect();
    let backend = Backend::new().unwrap();
    let selected = backend.select_with_state(&root, &state).unwrap();
    assert!(selected.watcher_active);
    let browse = backend.browse(selected.session).unwrap();
    assert_eq!(browse.notes.len(), 3);
    assert!(!browse.incomplete);
    assert!(browse.folders.contains(&"folder/empty".into()));
    let json = serde_json::to_value(&browse).unwrap();
    assert!(json["notes"][0].get("body").is_none());
    assert!(browse.diagnostics.is_empty());
    assert!(
        json["notes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|n| n.get("id").is_none() && n.get("ambiguous").is_none())
    );
    assert!(
        backend
            .open(selected.session, "plain.md")
            .unwrap()
            .body
            .contains("untouched")
    );
    assert_eq!(
        backend.open(selected.session, "metadata.md").unwrap().body,
        "bad"
    );
    assert!(
        backend
            .open(selected.session, "folder/a.md")
            .unwrap()
            .body
            .contains("needle")
    );
    let search = backend
        .search(
            selected.session,
            "needle".into(),
            Some("rust".into()),
            Some("folder".into()),
        )
        .unwrap();
    assert_eq!(search.hits.len(), 1);
    assert_eq!(search.hits[0].path, "folder/a.md");
    assert!(
        serde_json::to_value(&search).unwrap()["hits"][0]
            .get("id")
            .is_none()
    );
    assert!(
        serde_json::to_value(backend.open(selected.session, "folder/a.md").unwrap())
            .unwrap()
            .get("id")
            .is_none()
    );
    let persisted: std::path::PathBuf =
        serde_json::from_slice(&fs::read(state.join("config.json")).unwrap()).unwrap();
    assert_eq!(persisted, root);
    backend.shutdown();
    assert!(!backend.state().watcher_active);
    for (p, bytes) in before {
        assert_eq!(fs::read(root.join(p)).unwrap(), bytes);
    }
}
#[test]
fn links_are_contained_and_resolved_by_path() {
    let (_temp, root, state) = fixture();
    let backend = Backend::new().unwrap();
    let s = backend.select_with_state(&root, &state).unwrap().session;
    assert_eq!(
        backend
            .resolve_link(s, "folder/a.md", "a.md#heading")
            .unwrap()
            .path,
        "folder/a.md"
    );
    assert_eq!(
        relative_link("folder/a.md", "../folder/a.md").unwrap(),
        "folder/a.md"
    );
    for target in [
        "../../outside.md",
        "%2e%2e/%2e%2e/outside.md",
        "%252e%252e/outside.md",
        "/etc/passwd.md",
        "file:///etc/passwd.md",
        "javascript:alert(1)",
        "//host/a.md",
        "a%00.md",
        "..\\a.md",
        "a.md?x",
        "a%2f%2f.md",
    ] {
        assert!(
            backend.resolve_link(s, "folder/a.md", target).is_err(),
            "{target}"
        );
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("folder"), root.join("alias")).unwrap();
        assert!(backend.open(s, "alias/a.md").is_err());
        assert!(
            backend
                .resolve_link(s, "folder/a.md", "../alias/a.md")
                .is_err()
        );
        fs::remove_file(root.join("alias")).unwrap();
    }
    fs::write(
        root.join("folder/a.md"),
        "---\nid: arbitrary-id\n---\n# First",
    )
    .unwrap();
    fs::write(
        root.join("duplicate.md"),
        "---\nid: arbitrary-id\n---\n# Second",
    )
    .unwrap();
    assert_eq!(
        backend.resolve_link(s, "folder/a.md", "a.md").unwrap().path,
        "folder/a.md"
    );
    assert_eq!(
        backend
            .resolve_link(s, "folder/a.md", "../duplicate.md")
            .unwrap()
            .path,
        "duplicate.md"
    );
    assert_eq!(backend.open(s, "folder/a.md").unwrap().body, "# First");
    assert_eq!(backend.open(s, "duplicate.md").unwrap().body, "# Second");
    let browse = backend.browse(s).unwrap();
    assert!(!browse.incomplete);
    assert!(browse.diagnostics.is_empty());
    assert_eq!(browse.notes.len(), 2);
    for (query, path) in [("First", "folder/a.md"), ("Second", "duplicate.md")] {
        let result = backend.search(s, query.into(), None, None).unwrap();
        assert_eq!(result.hits.len(), 1);
        assert_eq!(result.hits[0].path, path);
    }
    let resolved = serde_json::to_value(
        backend
            .resolve_link(s, "folder/a.md", "../duplicate.md")
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        resolved,
        serde_json::json!({"session": s, "path": "duplicate.md"})
    );
    assert_eq!(
        fs::read_to_string(root.join("duplicate.md")).unwrap(),
        "---\nid: arbitrary-id\n---\n# Second"
    );
    assert!(backend.open(s, "../outside.md").is_err());
    assert!(backend.open(s, "/etc/passwd.md").is_err());
    assert!(backend.open(s, "path:folder/a.md").is_err());
}
#[test]
fn external_url_allowlist() {
    for url in [
        "https://example.org/a?q=1",
        "http://localhost/",
        "mailto:user@example.org?subject=Hello",
    ] {
        assert!(external_url(url).is_ok(), "{url}");
    }
    for url in [
        "javascript:alert(1)",
        "file:///etc/passwd",
        "data:text/html,test",
        "https://user:pass@example.org/",
        " https://example.org",
        "https://example.org/%0a",
        "mailto:",
        "https:\\example.org",
        "https://example.org\n",
    ] {
        assert!(external_url(url).is_err(), "{url}");
    }
}
#[test]
fn native_watcher_refreshes_current_disk_and_switch_releases_backend() {
    let (_temp, root, state) = fixture();
    let backend = Backend::new().unwrap();
    let selected = backend.select_with_state(&root, &state).unwrap();
    fs::write(root.join("folder/a.md"), "# Changed\nfresh disk\n").unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if backend.state().generation > selected.generation
            && backend
                .open(selected.session, "folder/a.md")
                .unwrap()
                .body
                .contains("fresh disk")
        {
            break;
        }
        assert!(Instant::now() < deadline, "watcher did not converge");
        thread::sleep(Duration::from_millis(20));
    }
    fs::rename(root.join("folder/a.md"), root.join("renamed.md")).unwrap();
    assert!(backend.open(selected.session, "folder/a.md").is_err());
    assert!(
        backend
            .open(selected.session, "renamed.md")
            .unwrap()
            .body
            .contains("fresh disk")
    );
    let paths: Vec<_> = backend
        .browse(selected.session)
        .unwrap()
        .notes
        .into_iter()
        .map(|n| n.path)
        .collect();
    assert!(paths.contains(&"renamed.md".into()));
    assert!(!paths.contains(&"folder/a.md".into()));
    let other = root.parent().unwrap().join("other");
    fs::create_dir(&other).unwrap();
    let switched = backend.select_with_state(&other, &state).unwrap();
    assert!(switched.session > selected.session);
    assert!(backend.browse(selected.session).is_err());
    backend.shutdown();
    // No process-global handle is retained: cleanly reopen the same root/watch.
    let reopened = Backend::new().unwrap();
    assert!(
        reopened
            .select_with_state(&root, &state)
            .unwrap()
            .watcher_active
    );
}
#[test]
fn concurrent_selections_are_serial_and_state_does_not_wait_for_core_lock() {
    let (_temp, root, state) = fixture();
    let backend = Arc::new(Backend::new().unwrap());
    let library = notes_core::Library::open(&root, &state, false).unwrap();
    let lock = library.lock().unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let threads: Vec<_> = (0..2)
        .map(|_| {
            let b = backend.clone();
            let r = root.clone();
            let s = state.clone();
            let barrier = barrier.clone();
            thread::spawn(move || {
                barrier.wait();
                b.select_with_state(&r, &s).unwrap().session
            })
        })
        .collect();
    barrier.wait();
    let start = Instant::now();
    for _ in 0..1000 {
        assert_eq!(backend.state().session, 0);
    }
    assert!(
        start.elapsed() < Duration::from_millis(200),
        "state blocked behind scan"
    );
    drop(lock);
    let mut sessions: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    sessions.sort();
    assert_eq!(sessions, vec![1, 2]);
    assert_eq!(backend.state().session, 2);
    assert!(backend.open(1, "folder/a.md").is_err());
}
#[test]
fn failed_selection_keeps_previous_session_and_configuration() {
    let (_temp, root, state) = fixture();
    let backend = Backend::new().unwrap();
    let before = backend.select_with_state(&root, &state).unwrap();
    let config = fs::read(state.join("config.json")).unwrap();
    assert!(
        backend
            .select_with_state(&root.join("missing"), &state)
            .is_err()
    );
    assert_eq!(backend.state().session, before.session);
    assert!(backend.state().watcher_active);
    assert_eq!(fs::read(state.join("config.json")).unwrap(), config);
}
