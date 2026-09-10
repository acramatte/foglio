//! Synthetic Linux-only measurement harness. No budget assertions or user config.
use notes_core::{
    Document, Library, revision,
    search::SearchQuery,
    watcher::{WatchOptions, Watcher},
};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    os::unix::fs::MetadataExt,
    path::Path,
    process::Command,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}
fn emit(value: Value) {
    println!("{value}");
    std::io::stdout().flush().unwrap();
}
fn memory(stage: &str) {
    let status = fs::read_to_string("/proc/self/status").unwrap();
    emit(
        json!({"kind":"memory","stage":stage,"values":status.lines().filter(|s| s.starts_with("VmRSS:") || s.starts_with("VmHWM:")).collect::<Vec<_>>()}),
    );
}
fn source(i: usize) -> String {
    format!(
        "---\ntags: [Work, topic{}]\n---\n# Synthetic note {i}\n\nCafé 日本語 lexical benchmark bucket{}\n\n- [ ] task\n\n| key | value |\n| --- | --- |\n| index | {i} |\n\n```rust\nlet value = {i};\n```\n{}",
        i % 20,
        i % 100,
        "Ordinary synthetic Markdown paragraph.\n".repeat(4 + i % 12)
    )
}
fn relative(i: usize) -> String {
    format!("folder{:02}/nested/note{i:05}.md", i % 50)
}
fn manifest(root: &Path, count: usize) -> String {
    let mut all = String::new();
    for i in 0..count {
        all.push_str(&relative(i));
        all.push_str(&revision(&fs::read(root.join(relative(i))).unwrap()).to_string());
    }
    revision(all.as_bytes()).to_string()
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args[1] == "warm-open" {
        let start = Instant::now();
        let lib = Library::open(Path::new(&args[2]), Path::new(&args[3]), false).unwrap();
        let opened = ms(start);
        let status = lib.status().unwrap();
        emit(
            json!({"kind":"sample","metric":"warm_process_open_status_ms","ms":ms(start),"handle_open_ms":opened,"parsed_notes":status.parsed_notes,"reused_notes":status.reused_notes,"indexed_notes":status.indexed_notes,"incomplete":status.incomplete}),
        );
        return;
    }
    if args[1] == "robustness" {
        let cases = vec![
            (
                "body_1m",
                "# Large\n".to_owned() + &"word ".repeat(209_716),
                true,
            ),
            (
                "body_8m",
                "# Large\n".to_owned() + &"word ".repeat(1_677_722),
                true,
            ),
            (
                "yaml_depth_100",
                format!("---\nx: {}0{}\n---\n", "[".repeat(100), "]".repeat(100)),
                false,
            ),
            (
                "yaml_alias",
                "---\ntags: &a [one]\ncustom: *a\n---\nbody".into(),
                false,
            ),
            ("duplicate_yaml", "---\nx: 1\nx: 2\n---\nbody".into(), false),
            ("unterminated_yaml", "---\nx: 1\nbody".into(), false),
            (
                "raw_html_retained_not_rendered",
                "# Raw\n<script>alert(1)</script>".into(),
                true,
            ),
        ];
        for (name, text, expected) in cases {
            for sample in 0..3 {
                let start = Instant::now();
                let result = Document::parse(text.as_bytes(), name);
                let elapsed = ms(start);
                let preserved = result.as_ref().is_ok_and(|d| d.source == text);
                emit(
                    json!({"kind":"robustness","case":name,"sample":sample,"bytes":text.len(),"ms":elapsed,"accepted":result.is_ok(),"expected_accepted":expected,"source_preserved":preserved,"error":result.as_ref().err().map(ToString::to_string)}),
                );
                assert_eq!(result.is_ok(), expected);
                if expected {
                    assert!(preserved);
                }
            }
            memory(name);
        }
        return;
    }
    let count: usize = args[1].parse().unwrap();
    assert!(count > 0);
    let base = Path::new(&args[2]);
    let tmp = tempfile::tempdir_in(base).unwrap();
    let root = tmp.path().join("notes");
    let state = tmp.path().join("state");
    let lib = Arc::new(Library::open(&root, &state, true).unwrap());
    let mut sizes = Vec::new();
    for i in 0..count {
        let p = root.join(relative(i));
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        let text = source(i);
        sizes.push(text.len());
        fs::write(p, text).unwrap();
    }
    sizes.sort_unstable();
    let before = manifest(&root, count);
    emit(
        json!({"kind":"corpus","notes":count,"bytes":sizes.iter().sum::<usize>(),"min_bytes":sizes[0],"median_bytes":sizes[count/2],"max_bytes":sizes[count-1],"manifest_before":before,"root":root,"cache":lib.cache_path().unwrap(),"cache_conditions":"fixture creation and manifest read prewarm page cache; no cache eviction; DB-cold means only SQLite removed"}),
    );
    memory("fixture");
    for sample in 0..3 {
        let cache = lib.cache_path().unwrap();
        if cache.exists() {
            fs::remove_file(cache).unwrap();
        }
        let start = Instant::now();
        let status = lib.reindex().unwrap();
        let elapsed = ms(start);
        emit(
            json!({"kind":"sample","metric":"db_cold_rebuild_ms","sample":sample,"ms":elapsed,"parsed_notes":status.parsed_notes,"indexed_notes":status.indexed_notes,"incomplete":status.incomplete}),
        );
        assert_eq!(status.indexed_notes, count);
        assert_eq!(status.parsed_notes, count);
        assert!(!status.incomplete);
        memory("rebuild");
    }
    let after = manifest(&root, count);
    emit(json!({"kind":"manifest","after":after,"unchanged":before==after}));
    assert_eq!(before, after);
    for sample in 0..5 {
        let start = Instant::now();
        let status = lib.status().unwrap();
        let elapsed = ms(start);
        emit(
            json!({"kind":"sample","metric":"warm_reconcile_ms","sample":sample,"ms":elapsed,"parsed_notes":status.parsed_notes,"reused_notes":status.reused_notes}),
        );
        assert_eq!(status.parsed_notes, 0);
        assert_eq!(status.reused_notes, count);
        let start = Instant::now();
        let output = Command::new(std::env::current_exe().unwrap())
            .arg("warm-open")
            .arg(&root)
            .arg(&state)
            .output()
            .unwrap();
        let wall = ms(start);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let mut record: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(record["parsed_notes"], 0);
        assert_eq!(record["reused_notes"], count);
        assert_eq!(record["incomplete"], false);
        record["sample"] = json!(sample);
        record["process_wall_ms"] = json!(wall);
        emit(record);
    }
    // The public one-shot API includes reconciliation and health checks. Keep
    // it separate from queries on an already acquired snapshot; desktop/CLI
    // currently pay this cost and must not inherit the prepared-query numbers.
    for sample in 0..10 {
        let start = Instant::now();
        let report = lib.search(&SearchQuery::literal("synthetic")).unwrap();
        emit(
            json!({"kind":"sample","metric":"search_api_common_ms","sample":sample,"ms":ms(start),"hits":report.hits.len(),"parsed_notes":report.status.parsed_notes}),
        );
        assert!(!report.hits.is_empty());
        assert_eq!(report.status.parsed_notes, 0);
    }
    let start = Instant::now();
    let session = lib.search_session().unwrap();
    emit(json!({"kind":"sample","metric":"search_session_acquire_ms","ms":ms(start)}));
    for i in 0..100 {
        for (family, query) in [
            ("selective", format!("bucket{}", i % 100)),
            ("common", "synthetic".into()),
            ("unicode", "日本語".into()),
            ("absent", "zznotincorpuszz".into()),
        ] {
            let q = SearchQuery::literal(&query);
            let start = Instant::now();
            let hits = session.search(&q).unwrap();
            let elapsed = ms(start);
            emit(
                json!({"kind":"sample","metric":format!("search_{family}_ms"),"sample":i,"query":query,"ms":elapsed,"hits":hits.len()}),
            );
            assert_eq!(hits.is_empty(), family == "absent");
        }
    }
    drop(session);
    memory("search");
    // Ordinary Linux cannot restore ctime: preserve size/mtime/inode explicitly, report ctime honestly.
    let p = root.join(relative(0));
    let old = fs::metadata(&p).unwrap();
    let new = source(0).replace("bucket0", "edited0");
    fs::write(&p, &new).unwrap();
    fs::File::open(&p)
        .unwrap()
        .set_times(
            fs::FileTimes::new()
                .set_accessed(old.accessed().unwrap())
                .set_modified(old.modified().unwrap()),
        )
        .unwrap();
    let changed = fs::metadata(&p).unwrap();
    let start = Instant::now();
    let status = lib.reindex().unwrap();
    let elapsed = ms(start);
    let session = lib.search_session().unwrap();
    let hits = session.search(&SearchQuery::literal("edited0")).unwrap();
    drop(session);
    let caught = hits.iter().any(|h| h.path == relative(0));
    emit(
        json!({"kind":"offline_reindex","ms":elapsed,"same_size":old.len()==changed.len(),"same_mtime":old.modified().unwrap()==changed.modified().unwrap(),"same_inode":old.ino()==changed.ino(),"same_ctime":(old.ctime(),old.ctime_nsec())==(changed.ctime(),changed.ctime_nsec()),"parsed_notes":status.parsed_notes,"caught_new_content":caught,"source_preserved":fs::read(&p).unwrap()==new.as_bytes()}),
    );
    assert!(caught);
    assert_eq!(old.len(), changed.len());
    assert_eq!(old.modified().unwrap(), changed.modified().unwrap());
    assert_eq!(status.parsed_notes, count);
    assert_eq!(fs::read(&p).unwrap(), new.as_bytes());
    let start = Instant::now();
    let w = Watcher::start(lib.clone(), WatchOptions::default()).unwrap();
    emit(
        json!({"kind":"sample","metric":"watcher_start_ms","ms":ms(start),"options":{"debounce_ms":75,"max_batch_delay_ms":500,"safety_interval_ms":30000,"hint_capacity":1024,"dirty_capacity":1024}}),
    );
    let sub = w.subscribe(64).unwrap();
    while sub.try_recv().is_some() {}
    let snapshot = w.snapshot();
    assert_eq!(snapshot.notes.len(), count);
    drop(snapshot);
    for sample in 0..20 {
        let text = format!("{new}\nWatcher revision {sample}\n");
        let expected = revision(text.as_bytes());
        while sub.try_recv().is_some() {}
        let start = Instant::now();
        let output = Command::new("python3")
            .arg("-c")
            .arg("import pathlib,sys; pathlib.Path(sys.argv[1]).write_bytes(sys.argv[2].encode())")
            .arg(&p)
            .arg(&text)
            .output()
            .unwrap();
        let writer_ms = ms(start);
        assert!(output.status.success());
        let settled = Instant::now();
        let mut converged = false;
        let mut generation = 0;
        while start.elapsed() < Duration::from_secs(120) {
            let mut event = false;
            while sub.try_recv().is_some() {
                event = true;
            }
            if event {
                let s = w.snapshot();
                generation = s.generation;
                if s.error.is_none()
                    && !s.status.incomplete
                    && s.notes.len() == count
                    && s.notes
                        .iter()
                        .any(|n| n.path == relative(0) && n.revision == expected)
                {
                    converged = true;
                    break;
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
        emit(
            json!({"kind":"sample","metric":"watcher_convergence_ms","sample":sample,"ms":ms(start),"after_writer_exit_ms":ms(settled),"writer_process_ms":writer_ms,"converged":converged,"generation":generation}),
        );
        assert!(converged, "watcher timeout retained in raw output");
        memory("watcher_sample");
    }
    w.shutdown().unwrap();
    memory("final");
    emit(json!({"kind":"complete","notes":count,"budget_gate":"not_evaluated_measure_first"}));
}
