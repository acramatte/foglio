use notes_desktop::{Backend, Mutation};
use std::fs;
fn code(result: notes_desktop::Result<Mutation>, expected: &str) {
    let error: serde_json::Value = serde_json::from_str(&result.unwrap_err()).unwrap();
    assert_eq!(error["code"], expected);
}

#[test]
fn recovery_copy_contract_and_session_guards() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("notes");
    fs::create_dir(&root).unwrap();
    let backend = Backend::new().unwrap();
    let state = temp.path().join("state");
    let s = backend.select_with_state(&root, &state).unwrap().session;
    let prefix = "\u{feff}---\r\nid: user-owned # comment\r\ntags: [old]\r\n---\r\n";
    let base = format!("{prefix}old\r\n");
    fs::write(root.join("a.md"), &base).unwrap();
    let note = backend.open(s, "a.md").unwrap();
    assert_eq!(note.source, base);
    let external = "---\ntags: 123\n---\nexternal";
    fs::write(root.join("a.md"), external).unwrap();
    code(
        backend.save_copy(
            s,
            "a.md",
            Some(&note.revision.to_string()),
            "lost.md",
            &base,
            "draft",
        ),
        "conflict",
    );
    code(
        backend.save_copy(s, "a.md", None, "lost.md", &base, "draft"),
        "conflict",
    );
    code(
        backend.save_copy(s, "a.md", Some("BAD"), "lost.md", &base, "draft"),
        "usage",
    );
    let observed = notes_core::revision(external.as_bytes()).to_string();
    let copied = backend
        .save_copy(
            s,
            "a.md",
            Some(&observed),
            "copy.md",
            &base,
            "edited\r\nmixed\n",
        )
        .unwrap();
    assert_eq!(copied.session, s);
    assert_eq!(copied.path, "copy.md");
    assert!(copied.file_committed);
    assert_eq!(
        copied.revision,
        Some(notes_core::revision(&fs::read(root.join("copy.md")).unwrap()).to_string())
    );
    assert_eq!(
        fs::read_to_string(root.join("copy.md")).unwrap(),
        format!("{prefix}edited\r\nmixed\n")
    );
    assert_eq!(fs::read_to_string(root.join("a.md")).unwrap(), external);
    code(
        backend.save_copy(s, "a.md", Some(&observed), "copy.md", &base, "lost"),
        "exists",
    );
    fs::remove_file(root.join("a.md")).unwrap();
    code(
        backend.save_copy(s, "a.md", Some(&observed), "lost.md", &base, "lost"),
        "conflict",
    );
    let error = match backend.open(s, "a.md") {
        Ok(_) => panic!("missing note opened"),
        Err(error) => error,
    };
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&error).unwrap()["code"],
        "not_found"
    );
    code(
        backend.save_copy(s, "a.md", None, "A.md", &base, "lost"),
        "path",
    );
    backend
        .save_copy(s, "a.md", None, "empty.md", &base, "")
        .unwrap();
    assert_eq!(fs::read_to_string(root.join("empty.md")).unwrap(), prefix);
    assert!(!root.join("a.md").exists());
    backend.select_with_state(&root, &state).unwrap();
    code(
        backend.save_copy(s, "a.md", None, "lost.md", &base, "lost"),
        "stale_session",
    );
    assert!(!root.join("lost.md").exists());
}

#[test]
fn copy_reports_committed_index_failure_without_losing_revision() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("notes");
    fs::create_dir(&root).unwrap();
    let state = temp.path().join("state");
    let backend = Backend::new().unwrap();
    let s = backend.select_with_state(&root, &state).unwrap().session;
    let library = notes_core::Library::open(&root, &state, false).unwrap();
    let cache = library.cache_path().unwrap();
    fs::remove_file(&cache).unwrap();
    fs::create_dir(&cache).unwrap();
    let copied = backend
        .save_copy(s, "missing.md", None, "copy.md", "base", "draft")
        .unwrap();
    assert!(copied.file_committed);
    assert!(!copied.warnings.is_empty());
    assert_eq!(
        copied.revision,
        Some(notes_core::revision(b"draft").to_string())
    );
    assert_eq!(fs::read_to_string(root.join("copy.md")).unwrap(), "draft");
}
