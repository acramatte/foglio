use notes_core::{
    Library, revision,
    watcher::{WatchOptions, Watcher},
};
use std::{
    fs,
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
fn writer(lib: &Library, script: &str) {
    let out = Command::new("python3")
        .arg("-c")
        .arg(script)
        .arg(lib.root())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
fn wait(w: &Watcher, test: impl Fn(&notes_core::watcher::WatchSnapshot) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let s = w.snapshot();
        if test(&s) {
            return;
        }
        assert!(Instant::now() < deadline, "watcher did not converge: {s:?}");
        thread::sleep(Duration::from_millis(15));
    }
}
#[test]
fn independent_process_lifecycle_and_directory_moves() {
    let (_t, lib) = fixture();
    let w = Watcher::start(lib.clone(), options()).unwrap();
    writer(
        &lib,
        "import pathlib,sys\np=pathlib.Path(sys.argv[1])/'deep'/'nested';p.mkdir(parents=True)\n(p/'a.md').write_text('---\\nid: 01ARZ3NDEKTSV4RRFFQ69G5FAV\\n---\\n# First\\n')",
    );
    wait(&w, |s| {
        s.notes.len() == 1 && s.notes[0].path == "deep/nested/a.md"
    });
    let first = w.snapshot().notes[0].clone();
    writer(
        &lib,
        "import pathlib,sys,os\np=pathlib.Path(sys.argv[1]);f=p/'deep/nested/a.md';t=f.with_suffix('.tmp');t.write_text(f.read_text().replace('First','Other'));os.replace(t,f);os.rename(p/'deep',p/'moved')",
    );
    wait(&w, |s| {
        s.notes.len() == 1
            && s.notes[0].path == "moved/nested/a.md"
            && s.notes[0].revision != first.revision
    });
    let actual = fs::read(lib.root().join("moved/nested/a.md")).unwrap();
    assert_eq!(w.snapshot().notes[0].revision, revision(&actual));
    writer(
        &lib,
        "import pathlib,sys,shutil\nshutil.rmtree(pathlib.Path(sys.argv[1])/'moved')",
    );
    wait(&w, |s| s.notes.is_empty());
    assert_eq!(lib.status().unwrap().indexed_notes, 0);
    w.shutdown().unwrap();
}
#[test]
fn self_writes_publish_committed_hashes_without_loops() {
    let (_t, lib) = fixture();
    let w = Watcher::start(lib.clone(), options()).unwrap();
    let sub = w.subscribe(16).unwrap();
    sub.try_recv(); // Initial invalidation/refetch barrier.
    let entry = lib.create("self.md", "# Self", &[]).unwrap();
    wait(&w, |s| s.notes.len() == 1);
    assert_eq!(w.snapshot().notes[0].revision, entry.document.revision);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(event) = sub.try_recv()
            && let notes_core::events::EventKind::NoteCreated { note } = event.kind
        {
            assert_eq!(note.revision, entry.document.revision);
            break;
        }
        assert!(Instant::now() < deadline, "no domain creation");
        thread::sleep(Duration::from_millis(10));
    }
    thread::sleep(Duration::from_millis(300));
    let generation = w.snapshot().generation;
    thread::sleep(Duration::from_millis(350));
    assert_eq!(
        generation,
        w.snapshot().generation,
        "self read/write feedback loop"
    );
    w.shutdown().unwrap();
}
