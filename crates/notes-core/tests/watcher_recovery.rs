use notes_core::{
    ErrorCode, Library,
    events::EventKind,
    revision,
    watcher::{WatchOptions, WatchSnapshot, Watcher},
};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    process::Command,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
fn fixture() -> (tempfile::TempDir, Arc<Library>) {
    let t = tempfile::tempdir().unwrap();
    let lib =
        Arc::new(Library::open(&t.path().join("notes"), &t.path().join("state"), true).unwrap());
    (t, lib)
}
fn options() -> WatchOptions {
    WatchOptions {
        debounce: Duration::from_millis(30),
        max_batch_delay: Duration::from_millis(100),
        safety_interval: Duration::from_secs(60),
        ..WatchOptions::default()
    }
}
fn wait(w: &Watcher, test: impl Fn(&WatchSnapshot) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let s = w.snapshot();
        if test(&s) {
            return;
        }
        assert!(Instant::now() < deadline, "no convergence: {s:?}");
        thread::sleep(Duration::from_millis(15));
    }
}
fn writer(lib: &Library, code: &str) {
    let out = Command::new("python3")
        .arg("-c")
        .arg(code)
        .arg(lib.root())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn drain(sub: &notes_core::watcher::Subscription) -> Vec<notes_core::events::Event> {
    std::iter::from_fn(|| sub.try_recv()).collect()
}

#[test]
fn atomic_burst_same_size_mtime_and_subscription_overflow() {
    let (_t, lib) = fixture();
    let mut opts = options();
    opts.hint_capacity = 1;
    opts.dirty_capacity = 1;
    let w = Watcher::start(lib.clone(), opts).unwrap();
    let slow = w.subscribe(1).unwrap();
    slow.try_recv();
    writer(
        &lib,
        "import pathlib,sys,os\np=pathlib.Path(sys.argv[1]); f=p/'a.md'\nfor i in range(150):\n t=p/'save.tmp';t.write_text('---\\nid: 01ARZ3NDEKTSV4RRFFQ69G5FAV\\n---\\n# %03d'%i);os.replace(t,f)\nfor i in range(12):\n (p/('n%d.md'%i)).write_text('---\\nid: 01ARZ3NDEKTSV4RRFFQ69G5F%02d\\n---\\n# Extra'%i)",
    );
    wait(&w, |s| s.notes.len() == 13 && !s.status.incomplete);
    thread::sleep(Duration::from_millis(150));
    assert!(matches!(
        slow.try_recv().unwrap().kind,
        EventKind::RescanRequired { .. }
    ));
    let before = w
        .snapshot()
        .notes
        .iter()
        .find(|n| n.path == "a.md")
        .unwrap()
        .revision
        .clone();
    writer(
        &lib,
        "import pathlib,sys,os\nf=pathlib.Path(sys.argv[1])/'a.md';s=f.stat();text=f.read_text();f.write_text(text.replace('149','XYZ'));os.utime(f,ns=(s.st_atime_ns,s.st_mtime_ns));assert f.stat().st_size==s.st_size and f.stat().st_mtime_ns==s.st_mtime_ns",
    );
    wait(&w, |s| {
        s.notes
            .iter()
            .any(|n| n.path == "a.md" && n.revision != before)
    });
    assert_eq!(
        w.snapshot()
            .notes
            .iter()
            .find(|n| n.path == "a.md")
            .unwrap()
            .revision,
        revision(&fs::read(lib.root().join("a.md")).unwrap())
    );
    // Overflow invalidation must not become an autonomous rescan feedback loop.
    thread::sleep(Duration::from_millis(300));
    let generation = w.snapshot().generation;
    thread::sleep(Duration::from_millis(300));
    assert_eq!(generation, w.snapshot().generation);
    w.shutdown().unwrap();
    assert!(slow.is_closed());
}
#[test]
fn pause_resume_manual_loss_periodic_and_restart() {
    let (_t, lib) = fixture();
    let mut opts = options();
    opts.safety_interval = Duration::from_millis(120);
    let w = Watcher::start(lib.clone(), opts).unwrap();
    let initial = w.snapshot().generation;
    wait(&w, |s| s.generation > initial); // Independent periodic pass, no file hints.
    w.pause();
    thread::sleep(Duration::from_millis(50));
    let generation = w.snapshot().generation;
    writer(
        &lib,
        "import pathlib,sys\n(pathlib.Path(sys.argv[1])/'delayed.md').write_text('---\\nid: 01ARZ3NDEKTSV4RRFFQ69G5FAV\\n---\\n# Delayed')",
    );
    thread::sleep(Duration::from_millis(250));
    assert_eq!(w.snapshot().generation, generation);
    w.resume();
    wait(&w, |s| s.notes.len() == 1 && !s.paused);
    let sub = w.subscribe(32).unwrap();
    sub.try_recv();
    let generation = w.snapshot().generation;
    w.report_event_loss();
    wait(&w, |s| s.generation > generation);
    assert!(drain(&sub).iter().any(
        |e| matches!(&e.kind, EventKind::RescanRequired { reason } if reason == "watcher_gap")
    ));
    let generation = w.snapshot().generation;
    w.rescan();
    wait(&w, |s| s.generation > generation);
    w.shutdown().unwrap();
    assert!(sub.is_closed());
    writer(
        &lib,
        "import pathlib,sys\nf=pathlib.Path(sys.argv[1])/'delayed.md';f.write_text(f.read_text().replace('Delayed','Offline'))",
    );
    let w = Watcher::start(lib.clone(), options()).unwrap();
    assert_eq!(
        w.snapshot().notes[0].revision,
        revision(&fs::read(lib.root().join("delayed.md")).unwrap())
    );
    w.shutdown().unwrap();
}
#[test]
fn duplicate_conflict_copy_and_temporary_disappearance() {
    let (_t, lib) = fixture();
    let entry = lib.create("one.md", "# Original", &[]).unwrap();
    let bytes = fs::read(lib.root().join("one.md")).unwrap();
    let w = Watcher::start(lib.clone(), options()).unwrap();
    let sub = w.subscribe(64).unwrap();
    sub.try_recv();
    writer(
        &lib,
        "import pathlib,sys,shutil\np=pathlib.Path(sys.argv[1]);shutil.copyfile(p/'one.md',p/'one.sync-conflict.md')",
    );
    wait(&w, |s| {
        s.status.ambiguous_notes == 2 && s.status.incomplete && s.notes.is_empty()
    });
    assert_eq!(
        fs::read(lib.root().join("one.sync-conflict.md")).unwrap(),
        bytes
    );
    assert!(
        drain(&sub)
            .iter()
            .any(|e| matches!(e.kind, EventKind::DiagnosticsChanged { .. }))
    );
    writer(
        &lib,
        "import pathlib,sys\n(pathlib.Path(sys.argv[1])/'one.sync-conflict.md').unlink()",
    );
    wait(&w, |s| s.notes.len() == 1 && !s.status.incomplete);
    assert_eq!(w.snapshot().notes[0].id, entry.document.id.unwrap());
    writer(
        &lib,
        "import pathlib,sys,time\np=pathlib.Path(sys.argv[1]);f=p/'one.md';data=f.read_bytes();f.unlink();time.sleep(.2);f.write_bytes(data)",
    );
    wait(&w, |s| {
        s.notes.len() == 1 && s.notes[0].revision == revision(&bytes) && !s.status.incomplete
    });
    assert_eq!(fs::read(lib.root().join("one.md")).unwrap(), bytes);
    w.shutdown().unwrap();
}
#[test]
fn permission_loss_is_unknown_not_deleted_and_recovers() {
    let (_t, lib) = fixture();
    lib.create("private/a.md", "# Secret", &[]).unwrap();
    let w = Watcher::start(lib.clone(), options()).unwrap();
    let sub = w.subscribe(64).unwrap();
    sub.try_recv();
    let directory = lib.root().join("private");
    writer(
        &lib,
        "import pathlib,sys,os\nos.chmod(pathlib.Path(sys.argv[1])/'private',0)",
    );
    // Restore even on assertion panic, including root CI (which must not pretend
    // permission denial was exercised).
    struct Restore(std::path::PathBuf);
    impl Drop for Restore {
        fn drop(&mut self) {
            fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700)).unwrap();
        }
    }
    let restore = Restore(directory.clone());
    assert!(
        fs::read_dir(&directory).is_err(),
        "run this gate unprivileged"
    );
    wait(&w, |s| s.status.incomplete && s.status.stale_notes == 1);
    assert!(
        !drain(&sub)
            .iter()
            .any(|e| matches!(e.kind, EventKind::NoteDeleted { .. }))
    );
    drop(restore);
    w.rescan();
    wait(&w, |s| !s.status.incomplete && s.notes.len() == 1);
    w.shutdown().unwrap();
}
#[test]
fn index_failure_after_commit_is_observable_and_never_rolls_back() {
    let (_t, lib) = fixture();
    let original = lib.create("a.md", "# Original", &[]).unwrap();
    let w = Watcher::start(lib.clone(), options()).unwrap();
    let sub = w.subscribe(64).unwrap();
    sub.try_recv();
    let db = lib.cache_path().unwrap();
    {
        let _lock = lib.lock().unwrap();
        fs::remove_file(&db).unwrap();
        fs::create_dir(&db).unwrap();
    }
    let error = lib
        .update("a.md", &original.document.revision, "# Committed")
        .unwrap_err();
    assert_eq!(error.code, ErrorCode::Committed);
    assert!(error.commit.unwrap().file_committed);
    wait(&w, |s| s.error.is_some() && s.status.incomplete);
    assert!(
        drain(&sub)
            .iter()
            .any(|e| matches!(e.kind, EventKind::IndexStateChanged { degraded: true, .. }))
    );
    let committed = fs::read(lib.root().join("a.md")).unwrap();
    assert!(String::from_utf8_lossy(&committed).contains("Committed"));
    {
        let _lock = lib.lock().unwrap();
        fs::remove_dir(&db).unwrap();
    }
    w.rescan();
    wait(&w, |s| {
        s.error.is_none() && !s.status.incomplete && s.notes[0].revision == revision(&committed)
    });
    assert_eq!(fs::read(lib.root().join("a.md")).unwrap(), committed);
    w.shutdown().unwrap();
}
#[test]
fn root_removal_recreation_and_symlink_are_not_followed() {
    let (t, lib) = fixture();
    lib.create("a.md", "# Safe", &[]).unwrap();
    let w = Watcher::start(lib.clone(), options()).unwrap();
    writer(
        &lib,
        "import pathlib,sys,shutil\np=pathlib.Path(sys.argv[1]);shutil.rmtree(p)",
    );
    wait(&w, |s| s.status.incomplete);
    fs::create_dir(lib.root()).unwrap();
    writer(
        &lib,
        "import pathlib,sys\n(pathlib.Path(sys.argv[1])/'new.md').write_text('---\\nid: 01ARZ3NDEKTSV4RRFFQ69G5FAV\\n---\\n# New')",
    );
    wait(&w, |s| {
        !s.status.incomplete
            && s.status.watcher_active
            && s.notes.len() == 1
            && s.notes[0].path == "new.md"
    });
    let outside = t.path().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("untouched.md"), "# Missing ID").unwrap();
    std::os::unix::fs::symlink(&outside, lib.root().join("link")).unwrap();
    wait(&w, |s| {
        s.status
            .diagnostics
            .iter()
            .any(|d| d.code == ErrorCode::Unsupported)
    });
    assert_eq!(
        fs::read_to_string(outside.join("untouched.md")).unwrap(),
        "# Missing ID"
    );
    assert!(
        !w.snapshot()
            .notes
            .iter()
            .any(|n| n.path.starts_with("link/"))
    );
    w.shutdown().unwrap();
}
#[test]
fn option_bounds_and_drop_close_subscriptions() {
    let (_t, lib) = fixture();
    let mut bad = options();
    bad.hint_capacity = 0;
    assert!(Watcher::start(lib.clone(), bad).is_err());
    let w = Watcher::start(lib, options()).unwrap();
    assert!(w.subscribe(0).is_err());
    let sub = w.subscribe(1).unwrap();
    drop(w);
    assert!(sub.is_closed());
}
