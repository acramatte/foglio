use notes_core::{ErrorCode, Library, revision};
use std::{fs, os::unix::fs::PermissionsExt};

fn setup() -> (tempfile::TempDir, Library) {
    let temp = tempfile::tempdir().unwrap();
    let lib = Library::open(&temp.path().join("notes"), &temp.path().join("state"), true).unwrap();
    (temp, lib)
}

#[test]
fn copy_preserves_base_metadata_and_exact_body_without_touching_source() {
    let (_temp, lib) = setup();
    let prefix =
        "\u{feff}---\r\nid: arbitrary # retained\r\ncustom: [one, two]\r\ntags: [old]\r\n---\r\n";
    let base = format!("{prefix}old body\r\n");
    let external = "---\ntags: 123\n---\nexternal malformed metadata";
    fs::write(lib.root().join("a.md"), external).unwrap();
    fs::set_permissions(lib.root().join("a.md"), fs::Permissions::from_mode(0o444)).unwrap();
    for (destination, body) in [("copy.md", "edited\r\nmixed\n"), ("empty.md", "")] {
        let commit = lib
            .save_copy(
                "a.md",
                Some(&revision(external.as_bytes())),
                destination,
                &base,
                body,
            )
            .unwrap_err()
            .commit
            .expect("invalid current metadata degrades index, not the committed copy");
        let expected = format!("{prefix}{body}");
        assert_eq!(
            fs::read_to_string(lib.root().join(destination)).unwrap(),
            expected
        );
        assert_eq!(commit.revision, Some(revision(expected.as_bytes())));
        assert!(commit.file_committed);
    }
    assert_eq!(
        fs::read_to_string(lib.root().join("a.md")).unwrap(),
        external
    );
    assert!(
        lib.scan()
            .unwrap()
            .notes
            .iter()
            .any(|e| e.path == "copy.md")
    );
}

#[test]
fn absent_source_copy_and_preconditions() {
    let (_temp, lib) = setup();
    let old = revision(b"old");
    for destination in ["a.md", "A.md"] {
        assert_eq!(
            lib.save_copy("a.md", None, destination, "old", "draft")
                .unwrap_err()
                .code,
            ErrorCode::Path
        );
    }
    assert_eq!(
        lib.save_copy("a.md", Some(&old), "copy.md", "old", "draft")
            .unwrap_err()
            .code,
        ErrorCode::Conflict
    );
    lib.save_copy("a.md", None, "copy.md", "old", "").unwrap();
    assert_eq!(fs::read(lib.root().join("copy.md")).unwrap(), b"");
    assert!(!lib.root().join("a.md").exists());
    assert_eq!(
        lib.save_copy("a.md", None, "copy.md", "old", "lost")
            .unwrap_err()
            .code,
        ErrorCode::Exists
    );
    fs::write(lib.root().join("a.md"), "external").unwrap();
    for expected in [None, Some(&old)] {
        assert_eq!(
            lib.save_copy("a.md", expected, "lost.md", "old", "lost")
                .unwrap_err()
                .code,
            ErrorCode::Conflict
        );
    }
    assert!(!lib.root().join("lost.md").exists());
    assert_eq!(
        lib.save_copy("../outside.md", None, "lost.md", "old", "lost")
            .unwrap_err()
            .code,
        ErrorCode::Path
    );
}
