use notes_core::{Document, ErrorCode, Library, filesystem, revision};
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::Path,
};

fn library(base: &Path) -> Library {
    Library::open(&base.join("notes"), &base.join("state"), true).unwrap()
}

#[test]
fn lifecycle_update_move_tags_and_stale_guards() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = library(tmp.path());
    let note = lib
        .create("a.md", "# Title\nBody", &["one".into()])
        .unwrap();
    let path = "a.md";
    lib.update(path, &note.document.revision, "# Updated\nNew body")
        .unwrap();
    for result in [
        lib.update(path, &note.document.revision, "stale"),
        lib.tag(path, &note.document.revision, "stale", true),
        lib.move_note(path, &note.document.revision, "wrong.md"),
        lib.delete(path, &note.document.revision),
    ] {
        assert_eq!(result.unwrap_err().code, ErrorCode::Conflict);
    }
    let current = lib.get(path).unwrap();
    assert_eq!(current.document.tags, ["one"]);
    assert_eq!(current.document.body, "# Updated\nNew body");
    lib.move_note(path, &current.document.revision, "deep/folder/renamed.md")
        .unwrap();
    assert_eq!(
        fs::read(lib.root().join("deep/folder/renamed.md")).unwrap(),
        current.document.source.as_bytes()
    );
    assert_eq!(lib.get(path).unwrap_err().code, ErrorCode::NotFound);
    let path = "deep/folder/renamed.md";
    let moved = lib.get(path).unwrap();
    lib.tag(path, &moved.document.revision, "Case", true)
        .unwrap();
    let tagged = lib.get(path).unwrap();
    assert_eq!(tagged.document.body, current.document.body);
    assert_eq!(tagged.document.tags, ["one", "Case"]);
    lib.delete(path, &tagged.document.revision).unwrap();
    assert!(lib.root().join("deep/folder").is_dir());
    assert_eq!(lib.get(path).unwrap_err().code, ErrorCode::NotFound);
}

#[test]
fn body_update_preserves_frontmatter_and_eof_closers() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = library(tmp.path());
    for close in ["---", "..."] {
        let source =
            format!("---\nid: 01ARZ3NDEKTSV4RRFFQ69G5FAV\nunknown:\n  a: b # retain\n{close}");
        fs::write(lib.root().join("a.md"), &source).unwrap();
        lib.update("a.md", &revision(source.as_bytes()), "new body")
            .unwrap();
        let updated = lib.get("a.md").unwrap();
        assert_eq!(updated.document.source, format!("{source}\nnew body"));
    }
}

#[test]
fn duplicate_metadata_is_ordinary_and_paths_are_literal() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = library(tmp.path());
    let source = "---\nid: not-a-ulid # preserve\n---\nbody";
    for path in ["a.md", "b.md"] {
        fs::write(lib.root().join(path), source).unwrap();
    }
    assert_eq!(lib.scan().unwrap().notes.len(), 2);
    for path in ["path:a.md", "id:a.md", "../a.md"] {
        assert_eq!(lib.get(path).unwrap_err().code, ErrorCode::Path);
    }
    lib.tag("a.md", &revision(source.as_bytes()), "one", true)
        .unwrap();
    assert!(
        lib.get("a.md")
            .unwrap()
            .document
            .source
            .contains("id: not-a-ulid # preserve")
    );
    lib.delete("b.md", &revision(source.as_bytes())).unwrap();
    assert_eq!(lib.scan().unwrap().notes.len(), 1);
}

#[test]
fn missing_path_mutations_never_retarget_identical_content() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = library(tmp.path());
    let source = "# Identical content";
    for path in ["a.md", "b.md"] {
        fs::write(lib.root().join(path), source).unwrap();
    }
    let expected = lib.get("a.md").unwrap().document.revision;
    fs::rename(lib.root().join("a.md"), lib.root().join("renamed.md")).unwrap();
    for result in [
        lib.update("a.md", &expected, "stale update"),
        lib.tag("a.md", &expected, "stale", true),
        lib.move_note("a.md", &expected, "wrong.md"),
        lib.delete("a.md", &expected),
    ] {
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
        for path in ["b.md", "renamed.md"] {
            assert_eq!(fs::read_to_string(lib.root().join(path)).unwrap(), source);
        }
        assert!(!lib.root().join("a.md").exists());
        assert!(!lib.root().join("wrong.md").exists());
    }
}

#[test]
fn oversized_create_update_and_scan_never_commit() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = library(tmp.path());
    let body = "a".repeat(filesystem::MAX_NOTE_BYTES + 1);
    assert!(lib.create("large.md", &body, &[]).is_err());
    assert!(!lib.root().join("large.md").exists());
    let note = lib.create("a.md", "body", &[]).unwrap();
    assert!(lib.update("a.md", &note.document.revision, &body).is_err());
    assert_eq!(
        fs::read(lib.root().join("a.md")).unwrap(),
        note.document.source.as_bytes()
    );
    fs::write(lib.root().join("import.md"), &body).unwrap();
    let report = lib.scan().unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.path == "import.md" && d.code == ErrorCode::Unsupported)
    );
    assert_eq!(
        fs::read_to_string(lib.root().join("import.md")).unwrap(),
        body
    );
}

#[test]
fn config_overlap_symlink_root_and_ancestors_are_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(Library::open(tmp.path(), &tmp.path().join("state"), true).is_err());
    fs::create_dir(tmp.path().join("real")).unwrap();
    symlink(tmp.path().join("real"), tmp.path().join("link")).unwrap();
    assert!(Library::open(&tmp.path().join("link"), &tmp.path().join("state"), false).is_err());
    assert!(
        Library::open(
            &tmp.path().join("link/sub"),
            &tmp.path().join("state"),
            true
        )
        .is_err()
    );
    assert!(!tmp.path().join("real/sub").exists());
}

#[test]
fn readonly_scan_and_hardlinked_mutations_are_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = library(tmp.path());
    let path = lib.root().join("import.md");
    fs::write(&path, "body").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
    assert!(!lib.scan().unwrap().incomplete);
    assert_eq!(fs::read_to_string(&path).unwrap(), "body");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    lib.scan().unwrap();
    let note = lib.get("import.md").unwrap();
    fs::hard_link(&path, tmp.path().join("outside.md")).unwrap();
    for result in [
        lib.update("import.md", &note.document.revision, "no"),
        lib.delete("import.md", &note.document.revision),
        lib.move_note("import.md", &note.document.revision, "moved.md"),
    ] {
        assert_eq!(result.unwrap_err().code, ErrorCode::Unsupported);
    }
    assert_eq!(fs::read(&path).unwrap(), note.document.source.as_bytes());
}

#[test]
fn invalid_paths_non_utf8_and_case_only_rename() {
    use std::os::unix::ffi::OsStringExt;
    let tmp = tempfile::tempdir().unwrap();
    let lib = library(tmp.path());
    for path in [
        "../a.md",
        "/a.md",
        "nested/../a.md",
        "a//b.md",
        "a/./b.md",
        "CON.md",
        "a.MD",
        "x\\y.md",
        "x:/y.md",
    ] {
        assert!(lib.create(path, "body", &[]).is_err(), "{path}");
    }
    let bad = std::ffi::OsString::from_vec(b"bad\xff.md".to_vec());
    fs::write(lib.root().join(&bad), "untouched").unwrap();
    assert!(
        lib.scan()
            .unwrap()
            .diagnostics
            .iter()
            .any(|d| d.code == ErrorCode::Path)
    );
    assert_eq!(fs::read(lib.root().join(&bad)).unwrap(), b"untouched");
    fs::remove_file(lib.root().join(&bad)).unwrap();
    let n = lib.create("case.md", "body", &[]).unwrap();
    lib.move_note("case.md", &n.document.revision, "Case.md")
        .unwrap();
    assert_eq!(
        lib.get("Case.md").unwrap().document.revision,
        n.document.revision
    );
    assert_eq!(
        Document::parse(n.document.source.as_bytes(), "x")
            .unwrap()
            .body,
        "body"
    );
}
