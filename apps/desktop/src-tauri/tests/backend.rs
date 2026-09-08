use notes_desktop::{Backend, external_url, relative_link};
use std::{
    fs,
    sync::{Arc, Barrier},
    thread,
    time::{Duration, Instant},
};
use tempfile::TempDir;
const ID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
fn fixture() -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("notes");
    let state = temp.path().join("state");
    fs::create_dir_all(root.join("folder/empty")).unwrap();
    fs::write(
        root.join("folder/a.md"),
        format!("---\nid: {ID}\ntags: [rust]\ncustom: preserved\n---\n# Alpha\nneedle\n"),
    )
    .unwrap();
    (temp, root, state)
}
#[test]
fn readonly_browse_search_open_and_selection_preserve_bytes() {
    let (_temp, root, state) = fixture();
    fs::write(root.join("unadopted.md"), "# Import\nuntouched\n").unwrap();
    fs::write(root.join("bad.md"), "---\nid: invalid\n---\nbad").unwrap();
    let before: Vec<_> = ["folder/a.md", "unadopted.md", "bad.md"]
        .into_iter()
        .map(|p| (p, fs::read(root.join(p)).unwrap()))
        .collect();
    let backend = Backend::new().unwrap();
    let selected = backend.select_with_state(&root, &state).unwrap();
    assert!(selected.watcher_active);
    let browse = backend.browse(selected.session).unwrap();
    assert_eq!(browse.notes.len(), 1);
    assert!(browse.incomplete);
    assert!(browse.folders.contains(&"folder/empty".into()));
    let json = serde_json::to_value(&browse).unwrap();
    assert!(json["notes"][0].get("body").is_none());
    assert!(browse.diagnostics.iter().any(|d| d.path == "unadopted.md"));
    assert!(
        backend
            .open(selected.session, ID)
            .unwrap()
            .body
            .contains("needle")
    );
    let _ = backend
        .search(
            selected.session,
            "needle".into(),
            Some("rust".into()),
            Some("folder".into()),
        )
        .unwrap();
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
fn links_are_contained_and_identity_checked() {
    let (_temp, root, state) = fixture();
    let backend = Backend::new().unwrap();
    let s = backend.select_with_state(&root, &state).unwrap().session;
    assert_eq!(
        backend
            .resolve_link(s, "folder/a.md", "a.md#heading")
            .unwrap()
            .id,
        ID
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
        assert!(
            backend
                .resolve_link(s, "folder/a.md", "../alias/a.md")
                .is_err()
        );
    }
    fs::copy(root.join("folder/a.md"), root.join("duplicate.md")).unwrap();
    assert!(backend.resolve_link(s, "folder/a.md", "a.md").is_err());
    assert!(backend.open(s, ID).is_err());
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
    fs::write(
        root.join("folder/a.md"),
        format!("---\nid: {ID}\n---\n# Changed\nfresh disk\n"),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if backend.state().generation > selected.generation
            && backend
                .open(selected.session, ID)
                .unwrap()
                .body
                .contains("fresh disk")
        {
            break;
        }
        assert!(Instant::now() < deadline, "watcher did not converge");
        thread::sleep(Duration::from_millis(20));
    }
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
    assert!(backend.open(1, ID).is_err());
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
