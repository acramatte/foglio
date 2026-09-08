use notes_core::{Library, search::SearchQuery};
use std::{fs, process::Command};

#[test]
fn init_select_scan_and_index_never_rewrite_markdown() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("notes");
    fs::create_dir(&root).unwrap();
    let sources = [
        ("plain.md", "# Plain\nuntouched"),
        (
            "arbitrary.md",
            "---\nid: arbitrary # user metadata\n---\n# Arbitrary",
        ),
        (
            "duplicate.md",
            "---\nid: arbitrary # user metadata\n---\n# Arbitrary",
        ),
        ("invalid.md", "---\ntags: scalar\n---\nuntouched"),
    ];
    for (path, source) in sources {
        fs::write(root.join(path), source).unwrap();
    }
    let result = Command::new(std::env::current_exe().unwrap())
        .args(["--ignored", "--exact", "isolated_init_child"])
        .env("FOGLIO_INIT_TEST_ROOT", &root)
        .env("XDG_CONFIG_HOME", temp.path().join("config"))
        .env("XDG_CACHE_HOME", temp.path().join("cache"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    for (path, source) in sources {
        assert_eq!(fs::read(root.join(path)).unwrap(), source.as_bytes());
    }
    assert_eq!(fs::read_dir(&root).unwrap().count(), sources.len());
}

#[test]
#[ignore = "isolated environment subprocess exercised by init test"]
fn isolated_init_child() {
    let root = std::path::PathBuf::from(std::env::var_os("FOGLIO_INIT_TEST_ROOT").unwrap());
    let (lib, report) = Library::init(&root).unwrap();
    assert_eq!(report.notes.len(), 3);
    assert!(report.incomplete);
    lib.select_existing().unwrap();
    assert_eq!(Library::resolve(None).unwrap().root(), root);
    assert_eq!(lib.scan().unwrap().notes.len(), 3);
    assert_eq!(lib.reindex().unwrap().indexed_notes, 3);
    assert_eq!(
        lib.search(&SearchQuery::literal("arbitrary"))
            .unwrap()
            .hits
            .len(),
        2
    );
    let new_root = root.parent().unwrap().join("new-notes");
    let (empty, report) = Library::init(&new_root).unwrap();
    assert!(report.notes.is_empty());
    assert!(empty.root().is_dir());
    assert_eq!(fs::read_dir(empty.root()).unwrap().count(), 0);
}

#[test]
fn create_is_plain_unless_tags_are_requested() {
    let temp = tempfile::tempdir().unwrap();
    let lib = Library::open(&temp.path().join("notes"), &temp.path().join("state"), true).unwrap();
    let plain = lib.create("plain.md", "hello", &[]).unwrap();
    assert_eq!(plain.document.source, "hello");
    assert_eq!(plain.document.title, "plain");
    let tagged = lib.create("tagged.md", "hello", &["work".into()]).unwrap();
    assert_eq!(tagged.document.source, "---\ntags: [\"work\"]\n---\nhello");
    assert!(!tagged.document.source.contains("id:"));
}
