# Phase 7 — local hardening evidence and release blockers

**P7-01 and the automated/local P7-02 slice are implemented and verified on Linux x86_64. P7-03 measurements and P7-04 Debian payload smoke are available; the Phase 7/v1 release gate is not complete.** No commits, pushes, hosted CI runs or releases were made.

The user authorized Phase 7, retained Linux-first scope, chose **measure first, then agree performance budgets**, and explicitly deferred actual second-device acceptance as blocked. Existing README frontmatter removal was preserved.

## Diagnostics and recovery (P7-01 / V14)

```sh
notes --library /absolute/library doctor
notes --library /absolute/library --json doctor
# Explicit derived-state repair, separate from diagnosis:
notes --library /absolute/library reindex
notes --library /absolute/library doctor
```

`doctor` fully reads source documents and inspects an existing cache, without opening a writable index, reconciling, creating state/locks, changing modes or rewriting notes. It reports malformed/unsupported sources, access/unknown-coverage problems, missing cache, stale/unindexed/orphan records, corrupt/unknown schema and inconsistent notes/tags/FTS tables. Unreadable paths are **unverified**, not proven orphans. Each finding has an action. Exit 0 means no findings; exit 5 means findings; operational/selection errors retain existing exit codes. JSON retains the normal envelope.

Cache inspection refuses sidecars and unsafe symlink/hard-link paths. It opens a percent-escaped immutable read-only SQLite URI, copies into memory, and executes integrity checks there (FTS integrity checking itself is a write). Fingerprint/sidecar checks detect observed concurrent cache changes. This is a best-effort observation, not an atomic filesystem snapshot: close writers and rerun for stable diagnosis. OS reads may change atime. Memory use includes the cache snapshot and parsed source corpus.

The existing `reindex` command is the sole explicit automated repair; it only reconstructs disposable derived state. No source-format repair, staging-file deletion, force overwrite, or ambiguous `--fix` is added. Unknown schemas remain refused. For structural damage which reindex cannot repair, close every client, identify only this library's app-owned cache, back it up if needed and remove that cache before reindexing. Never delete the library as a repair. The UI's existing diagnostics now include actionable metadata, permission, contention and cache guidance.

Seven core doctor tests and an actual CLI lifecycle test verify missing-state noncreation, unchanged directory/file bytes, malformed metadata, stale/orphan/missing coverage, permissions, schema/corruption, derived-table disagreement, special URI characters and unsafe cache files. Explicit rebuild returns a healthy diagnosis and preserves Markdown.

## Keyboard, focus and contention (P7-02 / V26)

- Ctrl+F focuses/selects literal library search; Down enters results, Up/Down/Home/End navigate, Escape returns to search, Enter/Space opens.
- Existing Ctrl+N/E/S create, switch source/preview and save remain. Modal dialogs suppress global shortcuts. Composition/repeated/Alt key events are ignored.
- Keyboard-help control, labelled/described native dialogs, cancellation focus restoration, stable result/filter focus, focusable preview, polite result counts instead of live-announcing whole documents, explicit empty-filter reset and result retry.
- Small-height layout retains a clickable last result at 400/600 px. Native testing caught and fixed the new help control consuming list space.
- One shared `Library` handle coordinates operations before taking the existing filesystem lock. Local waiting is capped at two seconds; reentry remains Busy. Separate handles/processes still use the immediate nonblocking filesystem lock. The guard spans the filesystem lock lifetime, preserving stale-write/commit behavior. This reduces same-process watcher/mutation failures; it is **not a contention-free or large-library responsiveness promise**.

`phase7_keyboard.py` uses real W3C key actions, no JS clicks/focus/edits or mocked IPC. It exercises onboarding, create, literal body search, edit/save, move, native Escape restoration, shortcut help/modal isolation and exact bytes using the shipped executable. DOM labels and keyboard operation are checked; **physical-display visual quality and actual assistive-technology/screen-reader qualification remain unverified**.

## Measurements (P7-03 / V25)

See [benchmark report](phase7-benchmarks.md), raw distributions and environment. Corpus creation/manifest reads prewarm the page cache; “DB-cold” is not physical cold storage. No numeric budget was accepted or silently relaxed.

The initial run exposed quadratic bulk rebuild behavior: deleting each note triggered an FTS scan of an unindexed path. The 50k process was explicitly terminated after 534 seconds total, while the populated-cache rebuild had not returned. Partial evidence is retained. Clearing FTS before deleting notes fixes the bulk path inside the same transaction; existing rollback/corruption/source-preservation regressions pass. Ordinary single-note FTS path lookups and full-library scans still have scaling costs.

## Packaging and tested target (P7-04 / V26–V27)

```sh
bash scripts/package-linux.sh
python3 tests/acceptance/phase7_package.py
```

The build script builds the release `notes` CLI from the same checkout, installs locked frontend dependencies, and produces `target/release/bundle/deb/Foglio_0.1.0_amd64.deb` with `/usr/bin/notes`, `/usr/bin/foglio-desktop`, icon and desktop entry. Build metadata is version 0.1.0, **not a completed v1 release**. The package has no install/remove scripts. Custom Cargo target directories are overridden deliberately to match the file map.

The smoke extracts into a disposable prefix, runs shipped CLI doctor/search/cache deletion/rebuild, runs native Phases 4/5/6/7 against the extracted executable, removes the payload and verifies the library manifest remains unchanged. This is **staged payload execution/removal, not a package-manager install/uninstall or fresh dependency-environment test**. Host GTK/WebKit libraries are reused. No system-wide installation or user-library changes were made. Bundling targets Debian/amd64 only; other distributions, architectures and OSes are not qualified by this test. See [external transfer/recovery instructions](sync.md).

The additional `bash tests/acceptance/phase7_install.sh` gate passed in a fresh Ubuntu 26.04 Docker userspace with package-manager dependency resolution, unprivileged native Phases 4/5/6/7, cache reconstruction and uninstall/source preservation. Evidence: `phase7-install-verified.log`. This is not a second device or a full independently booted OS. Earlier failed attempts are retained. Native timing exposed navigation controls remaining clickable during mutation acknowledgements; controls now reflect the busy state, including refreshed cards, with a regression test.

## Executed gates

All passed locally on the final integrated source:

- `cargo fmt --all -- --check`
- `cargo test --locked --workspace --all-features` (including subprocess-fixture parent tests)
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo build --locked -p notes-cli`
- `python3 tests/acceptance/phase1.py` — 12 scenarios
- `python3 tests/acceptance/phase2.py` — 4 scenarios
- `npm --prefix apps/desktop test` — 66 tests
- `bash scripts/package-linux.sh` — TypeScript check, Vite build, optimized Rust and Debian bundle
- `python3 tests/acceptance/phase7_package.py` — shipped CLI, Phase 4 native, 10 Phase 5 scenarios, 9 Phase 6 scenarios, keyboard workflow, cache/payload-removal manifest checks
- `cargo build --locked --release -p notes-core --example phase7_benchmark` followed by the documented run3 command — verified sample counts, 1k/50k source manifests, zero warm reparsing, actual separate-process writer convergence, and 21 robustness samples.

The CI definition now builds the Debian package and runs this staged-package harness, but **hosted execution remains pending**.

## Platform/security matrix and unresolved gates

| Boundary | Local evidence / limitation |
|---|---|
| Linux filesystem mutations | Existing fault injection, stale revisions, hard links/symlinks, ACL/xattr/permission refusal, no-clobber, same/different-device rename tests pass |
| Crash/transaction safety | Killed staged-writer and index rollback tests pass; no power-cut/storage-controller durability qualification |
| Watcher | Native separate writers, bursts, unknown permissions, root replacement, dropped hints, pause/resume and shared-handle coordination tested; actual OS suspend untested |
| Markdown/webview | Parser resource limits and hostile markup tests, real production-CSP WebKit/network-trap acceptance pass; no physical-display/AT qualification |
| Performance | Raw live-workstation measurements available; budgets undecided, large-corpus UI time-to-usable/search latency and physical-cold-storage behavior unmeasured |
| Packaging | Debian package install, dependency resolution, native acceptance, cache reconstruction and uninstall passed in a fresh same-host Ubuntu 26.04 container; separate-machine qualification remains blocked |
| External transfer | Local conflict-copy simulation passes; actual second-device/external-tool transfer explicitly blocked |
| Supported targets | Local Linux x86_64 only; no macOS/Windows, remote filesystem, provider-specific or cross-distro claim |

Do not mark Phase 7 or v1 complete until the remaining release gates have evidence. No sync engine, draft recovery/history, automatic merge, search-semantic changes, plugins or other roadmap scope was introduced.
