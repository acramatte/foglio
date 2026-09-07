# Phase 2: disposable index and lexical search

Phase 2 adds SQLite/FTS5 and the CLI maintenance/search surface. Markdown remains authoritative. No watcher, background daemon, desktop, or doctor is implemented. Hosted CI has not run for this uncommitted delivery.

## Commands

```sh
notes search 'meeting agenda'
notes search 'meeting agenda' --phrase
notes search 'meet' --prefix --tag Work --folder projects
notes search 'meeting' --limit 20 --json
notes status --json
notes rescan
notes reindex
```

All commands accept the existing `--library` and `--json` flags. `status` and `search` reconcile current metadata candidates. `rescan` hashes/parses every note; `reindex` additionally reconstructs all derived rows transactionally. Every index opening checks SQLite/FTS integrity before claiming healthy state. Maintenance does not adopt missing IDs or rewrite notes; use `init` for adoption.

Search uses SQLite `unicode61` tokens, ANDs literal tokens, and never exposes raw SQL or FTS syntax. Punctuation separates tokens; phrase mode requires token adjacency; prefix mode prefixes every token. Case/diacritics follow unicode61 for text matching. Tags filter by exact case-sensitive equality; folder filters match a literal directory component prefix, not SQL LIKE patterns. Rank is BM25 with title/body/path/tags weights 10/1/2/2 and a binary path tie-break. Limit defaults to 50, accepts 1–1000; query length is capped at 4096 bytes. Empty, punctuation-only and NUL queries are usage errors. Snippets are plain text, including any source HTML: future UI consumers must escape them, never insert them as HTML.

The existing envelope remains `{result, diagnostics, incomplete, error}`. Search result has `hits` and `status`; maintenance result is the status object. Exit codes remain 0 success, 1 operational/partial, 2 usage, 3 missing note, 4 conflict/ambiguity/busy. Status reports observed valid-path Markdown candidates, eligible indexed notes, retained stale records, ambiguous members, parsed/reused notes, cache rebuilding, diagnostics and `watcher_active: false` for this process. Counts do not imply inaccessible subtrees have been enumerated.

## State and recovery

CLI cache: `$XDG_CACHE_HOME/foglio/<SHA256(canonical-root)>/index.sqlite3`, falling back to `$HOME/.cache`. Config and cooperative locks remain under the Phase 1 config directory, outside disposable cache state. Core `Library::open(root, state, create)` uses `state/cache` for explicit, isolated callers. `resolve` and `init` use XDG cache selection. Cache/root overlap and symlink chains are refused.

Schema version 1 contains path-keyed `files` discovery records, unique-ID/path `notes`, case-sensitive foreign-keyed `tags`, and `notes_fts`. Missing databases migrate from scratch; corrupt SQLite headers/pages detected on opening or explicit integrity checks trigger coordinated removal of app-owned cache files and reconstruction. Unsupported future schema versions fail rather than being downgraded. Cache/sidecar symlinks, hard links and nonregular files are refused. Root-keyed cache directory mode is 0700 and DB mode 0600.

All Foglio index connections are scoped under the same nonblocking root lock, including search snapshots. SQLite uses DELETE journaling, FULL synchronous mode, foreign keys and a 250 ms busy timeout. WAL is deliberately not used: bounded busy responses and short-lived connection ownership keep cache replacement simple. A `SearchSession` supports repeated queries without rescanning, but holds the root lock until dropped; do not hold it across UI think time. Connection drops before lock release. Callers must use the same state/lock namespace for the same library, and must close processes before manually removing cache state. Uncooperative SQLite clients are outside this coordination contract.

A scan constructs current records before one transaction publishes changed metadata, tags and FTS. Changed/deleted eligible rows are removed before replacements so path/ID swaps cannot choose a winner. Every duplicate member is excluded. When one duplicate disappears, the unchanged survivor becomes eligible. Unknown subtrees preserve prior records as stale rather than deleting them; malformed metadata is not treated as healthy cached content. Conservatively, **any unknown identity suppresses all search eligibility**, since an unreadable/malformed file could conceal any duplicate ID. Complete access/metadata recovery restores eligibility. Non-note and missing-ID diagnostics also make the report incomplete.

File commit comes first. If subsequent cache work fails or scans are incomplete, mutations return error code `committed` with the actual `commit` object, including path/revision, `file_committed`, and `durability_confirmed`. This applies to create/update/tag/move/delete. Do not retry the original mutation blindly; inspect the file and run rescan/reindex. Cached text is never written back to notes. Adoption reports index failures in diagnostics without undoing successful adoption.

Warm reconciliation checks device/inode/size/mtime/ctime/mode and current readability, reusing cached parsed content for unchanged candidates. It still enumerates files and loads cached bodies into memory. Full rescan/reindex is the definitive content-checking recovery. Metadata equality cannot prove content equality against every external filesystem behavior. Watcher-touched paths and asynchronous reconciliation are Phase 3 work.

## Verification

Executed locally on Linux x86_64 / Rust 1.93.1:

- `cargo fmt --all -- --check`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo build --locked --workspace`
- `cargo test --locked --workspace --all-features`
- `python3 tests/acceptance/phase1.py`: 12 scenarios
- `python3 tests/acceptance/phase2.py`: 4 multi-scenario executable tests
- `/tmp/foglio-phase0-tools/bin/actionlint .github/workflows/ci.yml`
- `git diff --check`

| Scenarios | Evidence |
|---|---|
| V11 | Core and CLI scan/edit/move/delete; DB deletion/corrupt-header rebuild; complete note-byte manifest preserved |
| V12 | SQL-trigger injected transaction failure preserves previous tags; post-file-commit cache-open failures preserve create/tag/move/delete; core update failure; unreadable subtree retained stale then confirmed deletion |
| V13 | Independent title/body/path/tag matching, literal/phrase/prefix, punctuation and Unicode, hostile SQL/FTS strings, exact tags, component-aware wildcard folder names, deterministic ties, text snippets |
| V14 (status only) | Actual counts/parse reuse and inactive process watcher; doctor remains Phase 7 |
| V15 | Concurrent CLI create/reindex/rescan/search/status, held-lock bounded busy, snapshot/rebuild exclusion, existing concurrent adoption/no-clobber and stale-write suites |
| V25 baseline | Reproducible fixed 1k/50k datasets, raw rebuild/warm/search distributions, parse counts, memory and environment |

An intermittent existing parallel lock-release test exposed the close-only flock lifetime hazard around concurrent fork/exec. Locks now explicitly unlock on drop. The filesystem suite passed 30 consecutive parallel runs after the change. This is not an arbitrary power-loss or hostile external-writer proof; Phase 1 filesystem limitations remain.

## S05 baseline

Run:

```sh
cargo build --locked --release -p notes-core --example phase2_benchmark
python3 tests/acceptance/phase2_benchmark.py
```

Raw results and environment: [phase2-baseline.json](phase2-baseline.json). The corpus is explicitly synthetic: 50 nested folder groups, 20 tags, Unicode, tables/tasks/code and 365–808-byte notes. Three DB-cold rebuilds, five warm reconciliations and 100 core snapshot queries per corpus. **Temporary files reside on tmpfs; OS caches were not flushed. These are not cold SSD timings.** Search excludes reconciliation/UI rendering and uses moderately selective bucket terms; results are not a worst-case query guarantee.

| Notes | Rebuild p50/p95 (ms) | Warm reconciliation p50/p95 (ms) | Search p50/p95 (ms) | Warm body parses | Peak RSS (KiB) |
|---:|---:|---:|---:|---:|---:|
| 1,000 | 38.11 / 47.25 | 12.22 / 15.65 | 0.084 / 0.136 | 0 | 10,612 |
| 50,000 | 1596.83 / 1641.85 | 710.92 / 726.23 | 2.759 / 2.925 | 0 | 145,820 |

Machine: AMD Ryzen AI 9 HX 370, Linux 7.0.0-31-generic, release build. The proposed 100 ms warm search p95 is provisional and is not a release guarantee. Physical-storage cold behavior, large-note memory scaling, common-term worst cases, watcher convergence, desktop latency, other OSes and v1 release acceptance remain future gates.
