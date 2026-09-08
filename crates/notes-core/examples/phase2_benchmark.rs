//! Reproducible synthetic corpus benchmark, never opens user configuration.
use notes_core::{Library, search::SearchQuery};
use serde_json::json;
use std::{fs, time::Instant};
fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}
fn main() {
    let count: usize = std::env::args().nth(1).expect("count").parse().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let lib = Library::open(&tmp.path().join("notes"), &tmp.path().join("state"), true).unwrap();
    let mut bytes = 0usize;
    let mut sizes = Vec::new();
    for i in 0..count {
        let folder = lib.root().join(format!("folder{:02}/nested", i % 50));
        fs::create_dir_all(&folder).unwrap();
        let source = format!(
            "---\ntags: [Work, topic{}]\n---\n# Synthetic note {i}\n\nCafé 日本語 lexical benchmark bucket{}\n\n- [ ] task\n\n| key | value |\n| --- | --- |\n| index | {i} |\n\n```rust\nlet value = {i};\n```\n{}",
            i % 20,
            i % 100,
            "Ordinary synthetic Markdown paragraph.\n".repeat(4 + i % 12)
        );
        bytes += source.len();
        sizes.push(source.len());
        fs::write(folder.join(format!("note{i:05}.md")), source).unwrap();
    }
    let mut cold = Vec::new();
    for _ in 0..3 {
        let cache = lib.cache_path().unwrap();
        if cache.exists() {
            fs::remove_file(cache).unwrap();
        }
        let start = Instant::now();
        let status = lib.reindex().unwrap();
        cold.push(ms(start));
        assert_eq!(status.indexed_notes, count);
        assert_eq!(status.parsed_notes, count);
        assert!(!status.incomplete);
    }
    let mut warm = Vec::new();
    for _ in 0..5 {
        let start = Instant::now();
        let status = lib.status().unwrap();
        warm.push(ms(start));
        assert_eq!(status.parsed_notes, 0);
        assert_eq!(status.reused_notes, count);
    }
    let session = lib.search_session().unwrap();
    let mut search = Vec::new();
    for i in 0..100 {
        let q = SearchQuery::literal(&format!("bucket{}", i % 100));
        let start = Instant::now();
        let hits = session.search(&q).unwrap();
        search.push(ms(start));
        assert!(!hits.is_empty());
    }
    drop(session);
    let memory = fs::read_to_string("/proc/self/status")
        .unwrap()
        .lines()
        .find(|s| s.starts_with("VmHWM:"))
        .unwrap()
        .to_owned();
    sizes.sort_unstable();
    println!(
        "{}",
        json!({"notes":count,"corpus_bytes":bytes,"min_file_bytes":sizes[0],"median_file_bytes":sizes[count/2],"max_file_bytes":sizes[count-1],"cold_rebuild_ms":cold,"warm_reconcile_ms":warm,"warm_parsed_notes":0,"search_ms":search,"peak_memory":memory,"cache_conditions":"DB removed for cold rebuild; OS page cache not flushed; fixture generation prewarms files","watcher_convergence":"not applicable; Phase 3"})
    );
}
