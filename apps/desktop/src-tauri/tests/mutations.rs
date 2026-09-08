use notes_desktop::{Backend, Mutation};
use std::{fs, path::Path};

fn setup() -> (tempfile::TempDir, Backend, u64) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("notes");
    fs::create_dir(&root).unwrap();
    let backend = Backend::new().unwrap();
    let session = backend
        .select_with_state(&root, &temp.path().join("state"))
        .unwrap()
        .session;
    (temp, backend, session)
}
fn code(result: notes_desktop::Result<Mutation>, expected: &str) {
    let error: serde_json::Value = serde_json::from_str(&result.unwrap_err()).unwrap();
    assert_eq!(error["code"], expected, "{error}");
    assert!(!error["message"].as_str().unwrap().is_empty());
}
fn revision(result: &Mutation, root: &Path) -> String {
    assert!(result.file_committed);
    let revision = result.revision.clone().unwrap();
    assert_eq!(
        revision,
        notes_core::revision(&fs::read(root.join(&result.path)).unwrap()).to_string()
    );
    revision
}

#[test]
fn lifecycle_returns_literal_paths_and_exact_revisions() {
    let (temp, backend, s) = setup();
    let root = temp.path().join("notes");
    let created = backend
        .create(s, "folder/new.md", "New", "# New\nbody\n", &["rust".into()])
        .unwrap();
    assert_eq!(created.session, s);
    assert_eq!(created.path, "folder/new.md");
    assert!(created.warnings.is_empty());
    let first = revision(&created, &root);
    let saved = backend
        .save(
            s,
            &created.path,
            &first,
            "# Updated\n```unknown\n::: literal\n```\n",
        )
        .unwrap();
    let second = revision(&saved, &root);
    assert_ne!(first, second);
    assert_eq!(backend.open(s, &created.path).unwrap().tags, ["rust"]);
    let tagged = backend
        .change_tag(s, &created.path, &second, "new", true)
        .unwrap();
    let third = revision(&tagged, &root);
    assert_eq!(
        backend.open(s, &created.path).unwrap().tags,
        ["rust", "new"]
    );
    let untagged = backend
        .change_tag(s, &created.path, &third, "rust", false)
        .unwrap();
    let fourth = revision(&untagged, &root);
    let moved = backend
        .move_note(s, &created.path, &fourth, "renamed/note.md")
        .unwrap();
    assert_eq!(moved.path, "renamed/note.md");
    assert_eq!(revision(&moved, &root), fourth);
    assert!(!root.join(&created.path).exists());
    let deleted = backend.delete(s, &moved.path, &fourth).unwrap();
    assert_eq!(
        serde_json::to_value(&deleted).unwrap(),
        serde_json::json!({
            "session": s, "path": "renamed/note.md", "revision": null,
            "file_committed": true, "warnings": []
        })
    );
    assert!(!root.join(&moved.path).exists());
}

#[test]
fn stale_missing_and_malformed_revisions_never_overwrite_or_recreate() {
    let (temp, backend, s) = setup();
    let root = temp.path().join("notes");
    let created = backend.create(s, "a.md", "A", "# A\n", &[]).unwrap();
    let old = revision(&created, &root);
    fs::write(root.join("a.md"), "external edit").unwrap();
    for result in [
        backend.save(s, "a.md", &old, "lost"),
        backend.change_tag(s, "a.md", &old, "lost", true),
        backend.move_note(s, "a.md", &old, "lost.md"),
        backend.delete(s, "a.md", &old),
    ] {
        code(result, "conflict");
    }
    assert_eq!(
        fs::read_to_string(root.join("a.md")).unwrap(),
        "external edit"
    );
    for invalid in ["", "x", &"A".repeat(64), &format!(" {old}")] {
        code(backend.save(s, "a.md", invalid, "lost"), "usage");
    }
    fs::remove_file(root.join("a.md")).unwrap();
    for result in [
        backend.save(s, "a.md", &old, "lost"),
        backend.change_tag(s, "a.md", &old, "lost", true),
        backend.move_note(s, "a.md", &old, "lost.md"),
        backend.delete(s, "a.md", &old),
    ] {
        code(result, "not_found");
    }
    assert!(!root.join("a.md").exists());
    assert!(!root.join("lost.md").exists());
}

#[test]
fn collision_traversal_symlink_session_and_shutdown_guards() {
    let (temp, backend, s) = setup();
    let root = temp.path().join("notes");
    let a = backend.create(s, "a.md", "A", "# A\n", &[]).unwrap();
    let rev = revision(&a, &root);
    backend.create(s, "b.md", "B", "# B\n", &[]).unwrap();
    code(backend.create(s, "a.md", "lost", "lost", &[]), "exists");
    code(backend.move_note(s, "a.md", &rev, "b.md"), "exists");
    for path in [
        "../escape.md",
        "/absolute.md",
        "nested/../../escape.md",
        "a.txt",
        "a\\b.md",
    ] {
        code(backend.create(s, path, "bad", "bad", &[]), "path");
        code(backend.save(s, path, &rev, "bad"), "path");
        code(backend.move_note(s, "a.md", &rev, path), "path");
        code(backend.delete(s, path, &rev), "path");
        code(backend.change_tag(s, path, &rev, "bad", true), "path");
    }
    std::os::unix::fs::symlink(&root, root.join("alias")).unwrap();
    code(backend.create(s, "alias/new.md", "bad", "bad", &[]), "path");
    code(backend.save(s, "alias/a.md", &rev, "bad"), "path");
    fs::remove_file(root.join("alias")).unwrap();
    let next = backend
        .select_with_state(&root, &temp.path().join("state"))
        .unwrap()
        .session;
    for result in [
        backend.create(s, "lost.md", "lost", "lost", &[]),
        backend.save(s, "a.md", &rev, "lost"),
        backend.move_note(s, "a.md", &rev, "lost.md"),
        backend.delete(s, "a.md", &rev),
        backend.change_tag(s, "a.md", &rev, "lost", true),
    ] {
        code(result, "stale_session");
    }
    assert_eq!(fs::read_to_string(root.join("a.md")).unwrap(), "# A\n");
    assert_eq!(fs::read_to_string(root.join("b.md")).unwrap(), "# B\n");
    assert!(!root.join("lost.md").exists());
    backend.shutdown();
    code(backend.save(next, "a.md", &rev, "lost"), "closing");
}

#[test]
fn body_and_tag_updates_preserve_unknown_metadata_and_source_syntax() {
    let (temp, backend, s) = setup();
    let root = temp.path().join("notes");
    let prefix = "\u{feff}---\r\nid: arbitrary-user-value # retained\r\ncustom:\r\n  nested: [one, two] # comment\r\ntags: [old]\r\n---\r\n";
    fs::write(root.join("a.md"), format!("{prefix}# Original\r\n")).unwrap();
    let opened = backend.open(s, "a.md").unwrap();
    let body = "# Edited\r\n:::unsupported\r\n$math$ <!-- untouched -->\r\n";
    let saved = backend
        .save(s, "a.md", &opened.revision.to_string(), body)
        .unwrap();
    assert_eq!(
        fs::read_to_string(root.join("a.md")).unwrap(),
        format!("{prefix}{body}")
    );
    let tagged = backend
        .change_tag(s, "a.md", &revision(&saved, &root), "new", true)
        .unwrap();
    revision(&tagged, &root);
    let source = fs::read_to_string(root.join("a.md")).unwrap();
    assert!(source.starts_with("\u{feff}---\r\nid: arbitrary-user-value # retained\r\ncustom:\r\n  nested: [one, two] # comment\r\n"));
    assert!(source.ends_with(body));
}

#[test]
fn degraded_index_is_committed_success_for_every_mutation() {
    let (temp, backend, s) = setup();
    let root = temp.path().join("notes");
    let lib = notes_core::Library::open(&root, &temp.path().join("state"), false).unwrap();
    let cache = lib.cache_path().unwrap();
    // Block the derived SQLite path after watcher initialization, not source I/O.
    fs::remove_file(&cache).unwrap();
    fs::create_dir(&cache).unwrap();
    let created = backend.create(s, "a.md", "A", "# A\n", &[]).unwrap();
    let saved = backend
        .save(s, "a.md", &revision(&created, &root), "# saved\n")
        .unwrap();
    let tagged = backend
        .change_tag(s, "a.md", &revision(&saved, &root), "new", true)
        .unwrap();
    let moved = backend
        .move_note(s, "a.md", &revision(&tagged, &root), "b.md")
        .unwrap();
    let deleted = backend.delete(s, "b.md", &revision(&moved, &root)).unwrap();
    for result in [created, saved, tagged, moved, deleted] {
        assert!(result.file_committed);
        assert!(!result.warnings.is_empty(), "{result:?}");
    }
    assert!(!root.join("a.md").exists());
    assert!(!root.join("b.md").exists());
}

#[test]
fn creation_title_defaults_and_validation_preserve_supplied_source() {
    let (temp, backend, s) = setup();
    let root = temp.path().join("notes");
    backend.create(s, "a.md", "New title", "", &[]).unwrap();
    assert_eq!(
        fs::read_to_string(root.join("a.md")).unwrap(),
        "# New title\n"
    );
    backend
        .create(s, "b.md", "Unused title", "literal source", &[])
        .unwrap();
    assert_eq!(
        fs::read_to_string(root.join("b.md")).unwrap(),
        "literal source"
    );
    let spaced = backend
        .create_from_title(s, "System designs", "", &[])
        .unwrap();
    assert_eq!(spaced.path, "System-designs.md");
    assert_eq!(
        fs::read_to_string(root.join("System-designs.md")).unwrap(),
        "# System designs\n"
    );
    for title in ["", "  ", "line\nbreak", "tab\there"] {
        code(backend.create(s, "bad.md", title, "body", &[]), "usage");
    }
    assert!(!root.join("bad.md").exists());
    let unopened = Backend::new().unwrap();
    code(unopened.create(0, "a.md", "A", "", &[]), "no_library");
}

#[test]
fn unsupported_or_unreadable_source_is_not_replaced() {
    use std::os::unix::fs::PermissionsExt;
    let (temp, backend, s) = setup();
    let root = temp.path().join("notes");
    for (path, source, expected_code) in [
        ("bad.md", "---\ntags: 123\n---\nbody", "metadata"),
        ("readonly.md", "# read-only", "permission"),
    ] {
        fs::write(root.join(path), source).unwrap();
        if path == "readonly.md" {
            fs::set_permissions(root.join(path), fs::Permissions::from_mode(0o444)).unwrap();
        }
        let rev = notes_core::revision(source.as_bytes()).to_string();
        code(backend.save(s, path, &rev, "lost"), expected_code);
        assert_eq!(fs::read_to_string(root.join(path)).unwrap(), source);
    }
}

#[test]
fn concurrent_saves_do_not_acknowledge_two_writes_from_one_revision() {
    use std::sync::{Arc, Barrier};
    let (temp, backend, s) = setup();
    let root = temp.path().join("notes");
    let created = backend.create(s, "a.md", "A", "# A", &[]).unwrap();
    let rev = revision(&created, &root);
    let backend = Arc::new(backend);
    let start = Arc::new(Barrier::new(3));
    let workers: Vec<_> = ["first", "second"]
        .into_iter()
        .map(|body| {
            let backend = backend.clone();
            let start = start.clone();
            let rev = rev.clone();
            std::thread::spawn(move || {
                start.wait();
                backend.save(s, "a.md", &rev, body)
            })
        })
        .collect();
    start.wait();
    let mut successes = 0;
    for worker in workers {
        match worker.join().unwrap() {
            Ok(saved) => {
                revision(&saved, &root);
                successes += 1;
            }
            Err(error) => code(Err(error), "conflict"),
        }
    }
    assert_eq!(successes, 1);
}
