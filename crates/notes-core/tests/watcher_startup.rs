use notes_core::{
    Library,
    events::EventKind,
    revision,
    watcher::{WatchOptions, Watcher},
};
use std::{
    fs,
    process::Command,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

#[test]
fn independent_writer_overlaps_startup_and_domain_deltas_match_disk() {
    let temp = tempfile::tempdir().unwrap();
    let lib = Arc::new(
        Library::open(&temp.path().join("notes"), &temp.path().join("state"), true).unwrap(),
    );
    let mut child = Command::new("python3").arg("-c").arg("import pathlib,sys,time,os\np=pathlib.Path(sys.argv[1]);f=p/'a.md'\nfor i in range(100):\n t=p/'save.tmp';t.write_text('---\\nid: 01ARZ3NDEKTSV4RRFFQ69G5FAV\\n---\\n# %03d'%i);os.replace(t,f);time.sleep(.005)").arg(lib.root()).spawn().unwrap();
    let watcher = Watcher::start(
        lib.clone(),
        WatchOptions {
            safety_interval: Duration::from_secs(60),
            ..WatchOptions::default()
        },
    )
    .unwrap();
    assert!(child.wait().unwrap().success());
    let expected = revision(&fs::read(lib.root().join("a.md")).unwrap());
    let deadline = Instant::now() + Duration::from_secs(8);
    while !watcher
        .snapshot()
        .notes
        .iter()
        .any(|n| n.revision == expected)
    {
        assert!(Instant::now() < deadline, "startup did not converge");
        thread::sleep(Duration::from_millis(10));
    }
    let sub = watcher.subscribe(64).unwrap();
    sub.try_recv();
    let old = watcher.snapshot().notes[0].clone();
    let bytes = fs::read_to_string(lib.root().join("a.md"))
        .unwrap()
        .replace("099", "New");
    fs::write(lib.root().join("a.md"), &bytes).unwrap();
    let changed = await_event(&sub, |kind| matches!(kind, EventKind::NoteChanged { .. }));
    match changed {
        EventKind::NoteChanged {
            note,
            previous_revision,
        } => {
            assert_eq!(previous_revision, old.revision);
            assert_eq!(note.revision, revision(bytes.as_bytes()));
        }
        _ => unreachable!(),
    }
    fs::rename(lib.root().join("a.md"), lib.root().join("b.md")).unwrap();
    let removed = await_event(&sub, |kind| matches!(kind, EventKind::NoteDeleted { .. }));
    assert!(
        matches!(removed, EventKind::NoteDeleted { note } if note.path == "a.md" && note.revision == revision(bytes.as_bytes()))
    );
    let created = await_event(&sub, |kind| matches!(kind, EventKind::NoteCreated { .. }));
    assert!(
        matches!(created, EventKind::NoteCreated { note } if note.path == "b.md" && note.revision == revision(bytes.as_bytes()))
    );
    fs::remove_file(lib.root().join("b.md")).unwrap();
    let deleted = await_event(&sub, |kind| matches!(kind, EventKind::NoteDeleted { .. }));
    assert!(matches!(deleted, EventKind::NoteDeleted { note } if note.path == "b.md"));
    assert!(watcher.snapshot().notes.is_empty());
    watcher.shutdown().unwrap();
}

fn await_event(
    sub: &notes_core::watcher::Subscription,
    predicate: impl Fn(&EventKind) -> bool,
) -> EventKind {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        if let Some(event) = sub.try_recv()
            && predicate(&event.kind)
        {
            return event.kind;
        }
        assert!(Instant::now() < deadline, "domain event missing");
        thread::sleep(Duration::from_millis(10));
    }
}
