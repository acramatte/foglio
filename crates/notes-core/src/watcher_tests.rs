use super::*;

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
    let old = shared.snapshot.lock().unwrap().notes[0].revision.clone();
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
    let deadline = Instant::now() + Duration::from_secs(5);
    while watcher.snapshot().notes[0].revision == old {
        assert!(
            Instant::now() < deadline,
            "periodic recovery did not repair lost hints"
        );
        thread::sleep(TICK);
    }
    assert_eq!(
        watcher.snapshot().notes[0].revision,
        crate::revision(&std::fs::read(lib.root().join("a.md")).unwrap())
    );
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
