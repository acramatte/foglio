use notes_core::{
    Library,
    watcher::{WatchOptions, Watcher},
};
use std::sync::Arc;

#[test]
fn shared_library_status_tracks_each_registered_backend() {
    let temp = tempfile::tempdir().unwrap();
    let lib = Arc::new(
        Library::open(&temp.path().join("notes"), &temp.path().join("state"), true).unwrap(),
    );
    assert!(!lib.status().unwrap().watcher_active);
    let first = Watcher::start(lib.clone(), WatchOptions::default()).unwrap();
    assert!(lib.status().unwrap().watcher_active);
    let second = Watcher::start(lib.clone(), WatchOptions::default()).unwrap();
    first.shutdown().unwrap();
    assert!(lib.status().unwrap().watcher_active);
    second.shutdown().unwrap();
    assert!(!lib.status().unwrap().watcher_active);
}
