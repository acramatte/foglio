use notes_core::{Error, ErrorCode, filesystem as store, revision};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[test]
fn injected_failures_leave_old_bytes_and_clean_staging() {
    for point in [
        store::Stage::BeforeWrite,
        store::Stage::BeforeSync,
        store::Stage::BeforeReplace,
    ] {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.md");
        fs::write(&path, b"old").unwrap();
        let result =
            store::save_observed(&path, b"complete new", Some(&revision(b"old")), |stage| {
                if stage == point {
                    Err(Error::new(ErrorCode::Io, "injected fault"))
                } else {
                    Ok(())
                }
            });
        assert!(result.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"old");
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 1);
    }
}

#[test]
fn postcommit_fault_is_explicit_not_an_untouched_error() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("a.md");
    fs::write(&path, b"old").unwrap();
    let outcome = store::save_observed(&path, b"new", Some(&revision(b"old")), |stage| {
        if stage == store::Stage::AfterReplace {
            Err(Error::new(ErrorCode::Io, "directory sync fault"))
        } else {
            Ok(())
        }
    })
    .unwrap();
    assert!(outcome.file_committed);
    assert!(!outcome.durability_confirmed);
    assert_eq!(fs::read(&path).unwrap(), b"new");
}

#[test]
fn external_change_during_staging_and_permission_change_are_rejected() {
    for change_permissions in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.md");
        fs::write(&path, b"old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let result = store::save_observed(&path, b"ours", Some(&revision(b"old")), |stage| {
            if stage == store::Stage::BeforeReplace {
                if change_permissions {
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o640))?;
                } else {
                    fs::write(&path, b"external")?;
                }
            }
            Ok(())
        });
        assert_eq!(result.unwrap_err().code, ErrorCode::Conflict);
        assert_eq!(
            fs::read(&path).unwrap(),
            if change_permissions {
                &b"old"[..]
            } else {
                &b"external"[..]
            }
        );
    }
}

#[test]
fn no_clobber_create_and_move_preserve_all_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("a.md");
    let b = tmp.path().join("b.md");
    store::save(&a, b"first", None).unwrap();
    store::save(&b, b"second", None).unwrap();
    assert_eq!(
        store::save(&b, b"overwrite", None).unwrap_err().code,
        ErrorCode::Exists
    );
    assert_eq!(
        store::move_file(&a, &b, &revision(b"first"))
            .unwrap_err()
            .code,
        ErrorCode::Exists
    );
    assert_eq!(fs::read(&a).unwrap(), b"first");
    assert_eq!(fs::read(&b).unwrap(), b"second");
    let nested = tmp.path().join("new/sub/moved.md");
    assert!(
        store::move_file(&a, &nested, &revision(b"first"))
            .unwrap()
            .durability_confirmed
    );
    assert_eq!(fs::read(&nested).unwrap(), b"first");
    assert!(!a.exists());
}

#[test]
fn mode_owner_group_preserved_and_extended_attributes_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("private.md");
    fs::write(&path, b"old").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    let before = fs::metadata(&path).unwrap();
    store::save(&path, b"new", Some(&revision(b"old"))).unwrap();
    let after = fs::metadata(&path).unwrap();
    assert_eq!(
        (before.mode(), before.uid(), before.gid()),
        (after.mode(), after.uid(), after.gid())
    );
    rustix::fs::setxattr(
        &path,
        "user.foglio-test",
        b"retain",
        rustix::fs::XattrFlags::empty(),
    )
    .unwrap();
    assert_eq!(
        store::save(&path, b"no", Some(&revision(b"new")))
            .unwrap_err()
            .code,
        ErrorCode::Unsupported
    );
    assert_eq!(fs::read(&path).unwrap(), b"new");
}

#[test]
fn separate_handles_cannot_hold_same_cooperative_lock() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("lock");
    let guard = store::lock(&path).unwrap();
    assert_eq!(store::lock(&path).unwrap_err().code, ErrorCode::Busy);
    drop(guard);
    store::lock(&path).unwrap();
}

// Invoked by the parent test as a real separately killed writer process.
#[test]
#[ignore = "subprocess fixture, exercised by killed_staged_writer"]
fn staged_writer_child() {
    let base = std::path::PathBuf::from(std::env::var_os("FOGLIO_TEST_STAGE_ROOT").unwrap());
    store::save_observed(
        &base.join("a.md"),
        b"complete new",
        Some(&revision(b"old")),
        |stage| {
            if stage == store::Stage::BeforeReplace {
                fs::write(base.join("ready"), b"ready")?;
                loop {
                    std::thread::park();
                }
            }
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn killed_staged_writer_never_truncates_note() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("a.md");
    fs::write(&path, b"old").unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "staged_writer_child", "--ignored", "--nocapture"])
        .env_clear()
        .env("FOGLIO_TEST_STAGE_ROOT", tmp.path())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !tmp.path().join("ready").exists() && Instant::now() < deadline {
        if child.try_wait().unwrap().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let ready = tmp.path().join("ready").exists();
    let _ = child.kill();
    child.wait().unwrap();
    assert!(
        ready,
        "writer did not reach the post-sync/pre-replace boundary"
    );
    assert_eq!(fs::read(&path).unwrap(), b"old");
    let staged: Vec<_> = fs::read_dir(tmp.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "tmp"))
        .collect();
    assert_eq!(staged.len(), 1);
    assert_eq!(fs::read(&staged[0]).unwrap(), b"complete new");
}
