//! Linux platform tests. CI requires a writable /dev/shm on another device.
use notes_core::{ErrorCode, filesystem, revision};
use std::{fs, os::unix::fs::MetadataExt};

#[test]
fn cross_device_move_is_refused_without_copy_delete() {
    let local = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir_in("/dev/shm").unwrap();
    assert_ne!(
        fs::metadata(local.path()).unwrap().dev(),
        fs::metadata(other.path()).unwrap().dev(),
        "platform gate needs two different filesystems"
    );
    let source = local.path().join("source.md");
    let destination = other.path().join("destination.md");
    fs::write(&source, b"old").unwrap();
    let error = filesystem::move_file(&source, &destination, &revision(b"old")).unwrap_err();
    assert_eq!(error.code, ErrorCode::Unsupported);
    assert_eq!(fs::read(&source).unwrap(), b"old");
    assert!(!destination.exists());
}
