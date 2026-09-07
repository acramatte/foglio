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
    let id = note.document.id.as_ref().unwrap().as_str();
    lib.update(id, &note.document.revision, "# Updated\nNew body")
        .unwrap();
    for result in [
        lib.update(id, &note.document.revision, "stale"),
        lib.tag(id, &note.document.revision, "stale", true),
        lib.move_note(id, &note.document.revision, "wrong.md"),
        lib.delete(id, &note.document.revision),
    ] {
        assert_eq!(result.unwrap_err().code, ErrorCode::Conflict);
    }
    let current = lib.get(id).unwrap();
    assert_eq!(current.document.tags, ["one"]);
    assert_eq!(current.document.body, "# Updated\nNew body");
    lib.move_note(id, &current.document.revision, "deep/folder/renamed.md")
        .unwrap();
    assert_eq!(
        fs::read(lib.root().join("deep/folder/renamed.md")).unwrap(),
        current.document.source.as_bytes()
    );
    let moved = lib.get(id).unwrap();
    lib.tag(id, &moved.document.revision, "Case", true).unwrap();
    let tagged = lib.get(id).unwrap();
    assert_eq!(tagged.document.body, current.document.body);
    assert_eq!(tagged.document.id, note.document.id);
    assert_eq!(tagged.document.tags, ["one", "Case"]);
    lib.delete(id, &tagged.document.revision).unwrap();
    assert!(lib.root().join("deep/folder").is_dir());
    assert_eq!(lib.get(id).unwrap_err().code, ErrorCode::NotFound);
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
fn incomplete_metadata_cannot_hide_duplicate_identity() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = library(tmp.path());
    let note = lib.create("a.md", "body", &[]).unwrap();
    let id = note.document.id.as_ref().unwrap().as_str();
    fs::write(
        lib.root().join("bad.md"),
        format!("---\nid: {id}\ntags: scalar\n---\n"),
    )
    .unwrap();
    assert_eq!(
        lib.delete(id, &note.document.revision).unwrap_err().code,
        ErrorCode::Incomplete
    );
    assert_eq!(
        fs::read(lib.root().join("a.md")).unwrap(),
        note.document.source.as_bytes()
    );
    fs::write(lib.root().join("bad.md"), &note.document.source).unwrap();
    let report = lib.scan(false).unwrap();
    assert_eq!(report.notes.len(), 2);
    assert!(report.notes.iter().all(|e| e.ambiguous));
    assert!(lib.get("path:a.md").unwrap().ambiguous);
    assert_eq!(lib.get(id).unwrap_err().code, ErrorCode::Ambiguous);
    assert_eq!(
        lib.delete("path:a.md", &note.document.revision)
            .unwrap_err()
            .code,
        ErrorCode::Ambiguous
    );
    fs::remove_file(lib.root().join("bad.md")).unwrap();
    assert!(!lib.get(id).unwrap().ambiguous);
}

#[test]
fn oversized_create_update_and_adoption_never_commit() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = library(tmp.path());
    let body = "a".repeat(filesystem::MAX_NOTE_BYTES);
    assert!(lib.create("large.md", &body, &[]).is_err());
    assert!(!lib.root().join("large.md").exists());
    let note = lib.create("a.md", "body", &[]).unwrap();
    assert!(lib.update("a.md", &note.document.revision, &body).is_err());
    assert_eq!(
        fs::read(lib.root().join("a.md")).unwrap(),
        note.document.source.as_bytes()
    );
    fs::write(lib.root().join("import.md"), &body).unwrap();
    let report = lib.scan(true).unwrap();
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
fn readonly_adoption_and_hardlinked_mutations_are_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = library(tmp.path());
    let path = lib.root().join("import.md");
    fs::write(&path, "body").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
    assert!(lib.scan(true).unwrap().incomplete);
    assert_eq!(fs::read_to_string(&path).unwrap(), "body");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    lib.scan(true).unwrap();
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
        lib.scan(true)
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
    assert_eq!(lib.get("Case.md").unwrap().document.id, n.document.id);
    assert_eq!(
        Document::parse(n.document.source.as_bytes(), "x")
            .unwrap()
            .body,
        "body"
    );
}
