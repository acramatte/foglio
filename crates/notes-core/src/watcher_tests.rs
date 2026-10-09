use super::*;

#[test]
fn git_hints_are_filtered_before_queue_limits_and_keep_cross_boundary_renames() {
    let shared = Shared {
        snapshot: Mutex::new(WatchSnapshot::default()),
        subscribers: Mutex::new(Vec::new()),
        recovery: AtomicUsize::new(0),
        stopped: AtomicBool::new(false),
        paused: AtomicBool::new(false),
    };
    let root = Path::new("/library");
    let (tx, rx) = mpsc::sync_channel(1);
    let kind = NativeKind::Modify(notify::event::ModifyKind::Any);
    // Fill the queue first: metadata must neither consume capacity nor signal loss.
    let note = root.join("a.md");
    enqueue_hint(
        root,
        &tx,
        &shared,
        1,
        Ok(notify::Event::new(kind).add_path(note.clone())),
    );
    for _ in 0..100 {
        let event = notify::Event::new(kind)
            .add_path(root.join(".git/HEAD"))
            .add_path(root.join("nested/.git/config"));
        enqueue_hint(root, &tx, &shared, 1, Ok(event));
    }
    assert_eq!(shared.recovery.load(Ordering::Acquire), 0);
    assert_eq!(rx.try_recv().unwrap(), std::slice::from_ref(&note));
    assert!(rx.try_recv().is_err());

    let rename = NativeKind::Modify(notify::event::ModifyKind::Name(
        notify::event::RenameMode::Both,
    ));
    for paths in [
        [root.join(".git/hidden.md"), note.clone()],
        [note.clone(), root.join(".git/hidden.md")],
    ] {
        let event = notify::Event::new(rename)
            .add_path(paths[0].clone())
            .add_path(paths[1].clone());
        enqueue_hint(root, &tx, &shared, 1, Ok(event));
        assert_eq!(rx.try_recv().unwrap(), std::slice::from_ref(&note));
        assert_eq!(shared.recovery.load(Ordering::Acquire), 0);
    }
    for path in [".hidden/a.md", ".git-notes/a.md", ".git.md"] {
        enqueue_hint(
            root,
            &tx,
            &shared,
            1,
            Ok(notify::Event::new(kind).add_path(root.join(path))),
        );
        assert_eq!(rx.try_recv().unwrap(), [root.join(path)]);
    }
    // Genuine overflow is still observable after filtering.
    tx.try_send(vec![note.clone()]).unwrap();
    enqueue_hint(
        root,
        &tx,
        &shared,
        1,
        Ok(notify::Event::new(kind).add_path(note)),
    );
    assert_eq!(shared.recovery.load(Ordering::Acquire), LOST);
}

#[test]
fn git_hint_filter_preserves_backend_gap_recovery() {
    let shared = Shared {
        snapshot: Mutex::new(WatchSnapshot::default()),
        subscribers: Mutex::new(Vec::new()),
        recovery: AtomicUsize::new(0),
        stopped: AtomicBool::new(false),
        paused: AtomicBool::new(false),
    };
    let root = Path::new("/library");
    let (tx, rx) = mpsc::sync_channel(1);
    let kind = NativeKind::Modify(notify::event::ModifyKind::Any);
    let mut gap = notify::Event::new(kind).add_path(root.join(".git/HEAD"));
    gap.attrs.set_flag(notify::event::Flag::Rescan);
    for result in [
        Ok(gap),
        Ok(notify::Event::new(kind)),
        Ok(notify::Event::new(NativeKind::Any).add_path(root.join(".git/HEAD"))),
        Err(notify::Error::generic("injected backend error")),
    ] {
        enqueue_hint(root, &tx, &shared, 1, result);
        assert_eq!(shared.recovery.swap(0, Ordering::AcqRel), LOST);
        assert!(rx.try_recv().is_err());
    }
}

#[test]
fn periodic_pass_repairs_real_edit_with_all_native_hints_dropped() {
    let temp = tempfile::tempdir().unwrap();
    let lib = Arc::new(
        Library::open(&temp.path().join("notes"), &temp.path().join("state"), true).unwrap(),
    );
    lib.create("a.md", "# Before", &[]).unwrap();
    let shared = Arc::new(Shared {
        snapshot: Mutex::new(WatchSnapshot::default()),
        subscribers: Mutex::new(Vec::new()),
        recovery: AtomicUsize::new(0),
        stopped: AtomicBool::new(false),
        paused: AtomicBool::new(false),
    });
    // Fault injection: a real registered backend whose callback loses EVERY hint.
    let mut backend =
        RecommendedWatcher::new(|_: notify::Result<notify::Event>| {}, Config::default()).unwrap();
    backend.watch(lib.root(), RecursiveMode::Recursive).unwrap();
    lib.watcher_backends.fetch_add(1, Ordering::AcqRel);
    let native = RegisteredWatcher {
        _native: backend,
        library: lib.clone(),
    };
    reconcile(&lib, &shared, None);
    let (tx, rx) = mpsc::sync_channel(1);
    let state = shared.clone();
    let library = lib.clone();
    let worker = thread::spawn(move || {
        run(
            library,
            WatchOptions {
                safety_interval: Duration::from_millis(100),
                ..WatchOptions::default()
            },
            state.clone(),
            tx,
            rx,
            native,
        );
        state.finish();
    });
    let watcher = Watcher {
        shared,
        worker: Some(worker),
    };
    let output = std::process::Command::new("python3").arg("-c").arg("import pathlib,sys\np=pathlib.Path(sys.argv[1])/'a.md';p.write_text(p.read_text().replace('Before','After!'))").arg(lib.root()).output().unwrap();
    assert!(output.status.success());
    let expected = crate::revision(&std::fs::read(lib.root().join("a.md")).unwrap());
    let deadline = Instant::now() + Duration::from_secs(5);
    while watcher.snapshot().notes[0].revision != expected {
        assert!(
            Instant::now() < deadline,
            "periodic recovery did not observe the final external bytes"
        );
        thread::sleep(TICK);
    }
    assert_eq!(watcher.snapshot().notes[0].revision, expected);
    watcher.shutdown().unwrap();
}

#[test]
fn deterministic_reconciliation_hashes_content_and_overflow_is_sticky() {
    let temp = tempfile::tempdir().unwrap();
    let lib = Library::open(&temp.path().join("notes"), &temp.path().join("state"), true).unwrap();
    let shared = Arc::new(Shared {
        snapshot: Mutex::new(WatchSnapshot::default()),
        subscribers: Mutex::new(Vec::new()),
        recovery: AtomicUsize::new(0),
        stopped: AtomicBool::new(false),
        paused: AtomicBool::new(false),
    });
    let watcher = Watcher {
        shared: shared.clone(),
        worker: None,
    };
    reconcile(&lib, &shared, None);
    let sub = watcher.subscribe(1).unwrap();
    let stale = watcher.snapshot();
    let note = lib.create("one.md", "# First", &[]).unwrap();
    reconcile(&lib, &shared, None);
    // A snapshot fetched before consuming invalidation is not sufficient.
    assert!(stale.notes.is_empty());
    assert!(matches!(
        sub.try_recv().unwrap().kind,
        EventKind::RescanRequired { .. }
    ));
    assert_eq!(watcher.snapshot().notes[0].revision, note.document.revision);
    assert!(sub.try_recv().is_none());
    let second = lib.create("two.md", "# Second", &[]).unwrap();
    reconcile(&lib, &shared, None);
    let event = sub.try_recv().unwrap();
    assert!(
        matches!(event.kind, EventKind::NoteCreated { note: n } if n.revision == second.document.revision)
    );
    let bytes = std::fs::read(lib.root().join("one.md")).unwrap();
    std::fs::write(lib.root().join("one.md"), &bytes).unwrap();
    reconcile(&lib, &shared, None);
    assert!(
        sub.try_recv().is_none(),
        "same-content rewrite is not a domain change"
    );
    shared.publish(
        10,
        EventKind::RescanRequired {
            reason: "one".into(),
        },
    );
    shared.publish(
        11,
        EventKind::RescanRequired {
            reason: "two".into(),
        },
    );
    assert_eq!(shared.recovery.swap(0, Ordering::AcqRel), CLIENT);
    shared.publish(
        12,
        EventKind::RescanRequired {
            reason: "three".into(),
        },
    );
    assert_eq!(shared.recovery.load(Ordering::Acquire), 0);
    let invalidation = sub.try_recv().unwrap();
    assert_eq!(invalidation.generation, 11);
    assert!(
        matches!(invalidation.kind, EventKind::RescanRequired { reason } if reason == "subscriber_overflow")
    );
    assert!(sub.try_recv().is_none());
}

#[test]
fn quiet_rescans_keep_the_generation_and_real_changes_advance_it_once() {
    let temp = tempfile::tempdir().unwrap();
    let lib = Library::open(&temp.path().join("notes"), &temp.path().join("state"), true).unwrap();
    let shared = Arc::new(Shared {
        snapshot: Mutex::new(WatchSnapshot::default()),
        subscribers: Mutex::new(Vec::new()),
        recovery: AtomicUsize::new(0),
        stopped: AtomicBool::new(false),
        paused: AtomicBool::new(false),
    });
    let watcher = Watcher {
        shared: shared.clone(),
        worker: None,
    };
    reconcile(&lib, &shared, None);
    assert_eq!(watcher.snapshot().generation, 1);
    lib.create("a.md", "# One", &[]).unwrap();
    reconcile(&lib, &shared, None);
    assert_eq!(watcher.snapshot().generation, 2);
    // Safety rescans of a quiet library repeat reconciliation without paying a
    // refetch/re-render in consumers.
    reconcile(&lib, &shared, None);
    reconcile(&lib, &shared, None);
    assert_eq!(watcher.snapshot().generation, 2);
    // A same-content rewrite is not a domain change either.
    let bytes = std::fs::read(lib.root().join("a.md")).unwrap();
    std::fs::write(lib.root().join("a.md"), &bytes).unwrap();
    reconcile(&lib, &shared, None);
    assert_eq!(watcher.snapshot().generation, 2);
    assert_eq!(watcher.snapshot().notes.len(), 1);
}
