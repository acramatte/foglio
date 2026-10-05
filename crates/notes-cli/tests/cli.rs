use std::{fs, path::Path, process::Command};

fn run(args: &[&str]) -> std::process::Output {
    let sandbox = tempfile::tempdir().expect("create isolated sandbox");
    for directory in ["home", "config", "cache", "data", "library"] {
        fs::create_dir(sandbox.path().join(directory)).unwrap();
    }
    let note = sandbox.path().join("library/existing.md");
    fs::write(&note, "# Untouched\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_notes"))
        .args(args)
        .env_clear()
        .env("HOME", sandbox.path().join("home"))
        .env("XDG_CONFIG_HOME", sandbox.path().join("config"))
        .env("XDG_CACHE_HOME", sandbox.path().join("cache"))
        .env("XDG_DATA_HOME", sandbox.path().join("data"))
        .env("NO_COLOR", "1")
        .current_dir(sandbox.path().join("library"))
        .output()
        .expect("run actual notes executable");
    for directory in ["home", "config", "cache", "data"] {
        assert_eq!(
            fs::read_dir(sandbox.path().join(directory))
                .unwrap()
                .count(),
            0
        );
    }
    assert_eq!(fs::read_to_string(note).unwrap(), "# Untouched\n");
    assert_eq!(
        fs::read_dir(sandbox.path().join("library"))
            .unwrap()
            .count(),
        1
    );
    output
}

#[test]
fn help_describes_phase_one_without_opening_state() {
    for args in [&["--help"][..], &["-h"][..], &[][..]] {
        let output = run(args);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(text.contains("Usage: notes"));
        assert!(text.contains("Commands:"));
        assert!(text.contains("init"));
        assert!(text.contains("delete"));
    }
}

#[test]
fn version_matches_package() {
    let output = run(&["--version"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("notes {}\n", env!("CARGO_PKG_VERSION"))
    );
}

/// Run the executable against a prepared library without the strict
/// single-file assumptions of `run()`.
fn run_in_library(library: &Path, args: &[&str]) -> std::process::Output {
    let sandbox = tempfile::tempdir().expect("create isolated sandbox");
    Command::new(env!("CARGO_BIN_EXE_notes"))
        .args(args)
        .env_clear()
        .env("HOME", sandbox.path().join("home"))
        .env("XDG_CONFIG_HOME", sandbox.path().join("config"))
        .env("XDG_CACHE_HOME", sandbox.path().join("cache"))
        .env("XDG_DATA_HOME", sandbox.path().join("data"))
        .env("NO_COLOR", "1")
        .arg("--library")
        .arg(library)
        .output()
        .expect("run actual notes executable")
}

#[test]
fn git_backed_library_indexes_notes_and_creates_cleanly() {
    let sandbox = tempfile::tempdir().unwrap();
    let library = sandbox.path().join("library");
    fs::create_dir_all(&library).unwrap();
    fs::write(library.join("existing.md"), "# Untouched\n").unwrap();
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(&library)
            .output()
            .expect("run git")
            .status
            .success()
    );
    // Nested repository metadata and a worktree pointer file.
    fs::create_dir_all(library.join("nested/repo/.git/refs")).unwrap();
    fs::write(library.join("nested/repo/.git/config"), "[core]\n").unwrap();
    fs::create_dir_all(library.join(".wt")).unwrap();
    fs::write(library.join(".wt/.git"), "gitdir: ../.git/worktrees/wt\n").unwrap();

    let created = run_in_library(&library, &["--json", "new", "Git Note"]);
    assert!(
        created.status.success(),
        "stdout: {}stderr: {}",
        String::from_utf8_lossy(&created.stdout),
        String::from_utf8_lossy(&created.stderr)
    );
    assert_eq!(
        fs::read_to_string(library.join("Git-Note.md")).unwrap(),
        "# Git Note\n"
    );

    let status = run_in_library(&library, &["--json", "status"]);
    assert!(
        status.status.success(),
        "status must be complete: {status:?}"
    );
    let stdout = String::from_utf8(status.stdout).unwrap();
    assert!(stdout.contains("\"discovered_notes\":2"), "{stdout}");
    assert!(stdout.contains("\"incomplete\":false"), "{stdout}");
    assert!(library.join(".git/HEAD").exists());
    assert!(library.join(".wt/.git").exists());
    assert!(library.join("nested/repo/.git/config").exists());
}

#[test]
fn unsupported_commands_and_flags_are_usage_errors() {
    for argument in ["search", "sync", "--unknown"] {
        let output = run(&[argument]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr).unwrap().contains("error:"));
    }
}
