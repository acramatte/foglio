# Phase 7 benchmark evidence

**Measure first; no release budgets have been accepted.** These are synthetic fixtures on a live Linux workstation, not user data or an idle-machine guarantee.

## Reproduce

```sh
cargo build --locked --release -p notes-core --example phase7_benchmark
python3 tests/acceptance/phase7_benchmark.py --output docs/phase7-benchmark-new --base /path/on/test/storage
```

The output directory must not already exist. The corpus/state lives in a newly created temporary directory under the specified existing base; no existing library is used. Raw JSONL, stderr and load samples survive failures. Process interruption can leave its disposable corpus behind.

## Final measurement run

[Run 3 environment](phase7-benchmark-run3/environment.json) · [summary](phase7-benchmark-run3/summary.json) · [verified counts](phase7-benchmark-run3/verified.json)

AMD Ryzen AI 9 HX 370; Linux x86_64/glibc 2.43; ext4 on `/dev/mapper/ubuntu--vg-ubuntu--lv`. Rust 1.93.1 release example. Environment files include CPU/RAM, block devices, tool versions, executable/lockfile/harness hashes and source revision/dirty status. Continuous samples record workstation load. Source creation and manifest reads prewarm page cache; no privileged cache eviction was attempted. DB-cold means only the SQLite file was removed, **not physical-cold storage**.

| Metric (sample count) | 1k p50 / p95, ms | 50k p50 / p95, ms |
|---|---:|---:|
| DB-cold rebuild (3) | 65.75 / 78.76 | 1757.59 / 1801.39 |
| Warm process open + status (5) | 22.78 / 25.48 | 782.37 / 828.11 |
| Public Library::search, common word (10) | 14.68 / 27.35 | 909.72 / 999.32 |
| Prepared selective search (100) | 0.10 / 0.13 | 2.31 / 3.42 |
| Prepared common-word search (100) | 1.62 / 1.82 | 100.20 / 153.88 |
| Prepared Unicode search (100) | 1.27 / 1.46 | 93.58 / 152.58 |
| Prepared absent-word search (100) | 0.08 / 0.15 | 0.30 / 0.40 |
| Separate-writer convergence (20) | 147.30 / 156.61 | 1577.96 / 1711.60 |

Prepared queries hold a pre-acquired search snapshot. Their timings exclude reconciliation, health checks and UI rendering. The public one-shot API includes those core costs and is the relevant warning for the existing CLI/desktop call path. **Do not present the selective-query number as end-to-end search latency.** p95 uses nearest rank; with only 3/5/10 samples it is the maximum, not a strong tail-confidence estimate.

- 1,000 notes: 552,946 source bytes; 332–769 bytes/note. Final memory: VmHWM:    17656 kB, VmRSS:    17656 kB. Populated-cache forced rebuild/offline-edit check: 62.27 ms (one sample).
- 50,000 notes: 27,961,046 source bytes; 332–775 bytes/note. Final memory: VmHWM:   313692 kB, VmRSS:   292196 kB. Populated-cache forced rebuild/offline-edit check: 3469.38 ms (one sample).

Both corpora retain source manifests across rebuilds. All warm status/process-open samples parse zero bodies and reuse the full count. Forced reindex finds the changed content with unchanged size, mtime and inode; ctime changed and is explicitly reported, so this is not evidence of a forged all-metadata-equal inode. Each of 20 settled edits per corpus is written by a separate Python process and verified by the watcher snapshot revision, not merely a generation increment. Convergence includes writer process startup; per-sample post-exit duration is also retained.

The separate robustness corpus has 21 samples: 1 MiB/8 MiB bodies, excessive YAML depth, aliases, duplicate keys, unterminated metadata and raw HTML. Accepted sources are checked for exact preservation; raw HTML retention is not a claim that it is rendered (native security tests cover that boundary).

## Retained attempts and discovered bottleneck

- [Run 1](phase7-benchmark-run1/summary.json): complete 1k measurements; 50k completed rebuild/warm/search samples but was explicitly SIGTERM-terminated after 534.13 seconds total while populated-cache forced rebuild was still running. This is an interrupted measurement, not a completed rebuild time. Raw outputs/load and failure status remain, with no fabricated watcher samples. The benchmark executable predates shared-handle coordination and the rebuild fix.
- `notes_delete` scans FTS by its unindexed path. Bulk `DELETE FROM notes` therefore repeatedly scanned a populated FTS table. Empty FTS first, then notes/files, within the same transaction; existing rollback/source-preservation tests still pass.
- [Run 2](phase7-benchmark-run2/summary.json): complete post-fix original harness. It retains distributions rather than replacing run 1.
- Run 3 adds the public one-shot search API distribution, avoiding an optimistic prepared-query-only conclusion. It is the report table above. Common/Unicode tail latency is worse than run 2; slow samples are retained, not filtered.

## Release interpretation

No budgets were silently approved or adjusted. The old proposed 100 ms search threshold is not supported by the public API measurements; even prepared common/Unicode search tails exceed it in run 3. Watcher convergence is measured, not an idle-machine or UI guarantee. Physical-cold-storage, large-library UI time-to-usable/rendering/memory, actual suspend, network filesystems, other OSes and second-device transfer remain unqualified. P7-03 is **measured but not release-qualified** pending budget decisions and missing evidence.
