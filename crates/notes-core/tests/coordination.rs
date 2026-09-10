use notes_core::{ErrorCode, Library};
use std::{
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

#[test]
fn shared_handle_waits_for_in_process_operation_but_reentry_stays_busy() {
    let temp = tempfile::tempdir().unwrap();
    let lib = Arc::new(
        Library::open(&temp.path().join("notes"), &temp.path().join("state"), true).unwrap(),
    );
    let held = lib.lock().unwrap();
    assert_eq!(lib.lock().unwrap_err().code, ErrorCode::Busy);
    let (started, ready) = mpsc::channel();
    let (done, result) = mpsc::channel();
    let worker = lib.clone();
    let join = thread::spawn(move || {
        started.send(()).unwrap();
        done.send(worker.create("after.md", "# After\n", &[]).map(|_| ()))
            .unwrap();
    });
    ready.recv().unwrap();
    assert!(
        result.recv_timeout(Duration::from_millis(100)).is_err(),
        "operation must wait, not report spurious Busy"
    );
    drop(held);
    result
        .recv_timeout(Duration::from_secs(3))
        .unwrap()
        .unwrap();
    join.join().unwrap();
    assert_eq!(lib.get("after.md").unwrap().document.body, "# After\n");
}

#[test]
fn separate_handles_still_use_the_nonblocking_filesystem_lock() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("notes");
    let state = temp.path().join("state");
    let first = Library::open(&root, &state, true).unwrap();
    let other = Library::open(&root, &state, false).unwrap();
    let _held = first.lock().unwrap();
    assert_eq!(other.lock().unwrap_err().code, ErrorCode::Busy);
}
