use std::{fs, process::Command};

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

#[test]
fn unsupported_commands_and_flags_are_usage_errors() {
    for argument in ["search", "sync", "--unknown"] {
        let output = run(&[argument]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr).unwrap().contains("error:"));
    }
}
