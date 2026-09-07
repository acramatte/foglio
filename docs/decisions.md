# Decision register

Status meanings: **Established** comes from the supplied brief; **Proposed** is a concrete planning default that needs acceptance before implementation; **Spike** needs technical evidence. Phase 0 bootstrap execution is recorded in [tasks](tasks.md); the later-phase spikes remain unrun.

## Established decisions

| ID | Decision | Rationale / source |
|---|---|---|
| D01 | Local Markdown files are authoritative; SQLite/FTS5 is disposable | Portability and rebuild, §§2, 18–20 |
| D02 | Rust headless core shared directly by CLI and Tauri/TypeScript | Reuse without daemon/IPC, §§4–5 |
| D03 | One library, physical directories, minimal YAML `id` and `tags` | §§6–11 |
| D04 | ULID IDs independent of path | Suggested in §9, required by first implementation task §43; examples in brief are illustrative, not valid fixtures |
| D05 | External filesystem sync and external editing | No distributed sync machinery, §§27–30 |
| D06 | Conservative file preservation and no silent duplicate-ID repair | §§23, 32 |
| D07 | Incremental phases; first delivery is Phase 0–1 only | §§36, 42–43 |
| D08 | No speculative sync/encryption/daemon/plugin abstractions | §§3, 42 |

## Accepted Phase 0 decisions

The user approved these choices before implementation:

| ID | Decision | Rationale / gate |
|---|---|---|
| D09 | Product Foglio; crates `notes-core` / `notes-cli`; binary `notes` | Preserve source command contracts; P0-01 accepted |
| D18 | Linux first; macOS/Windows support only after their filesystem and packaging gates pass | Avoid unverified platform claims; P0-01 accepted |
| D21 | MIT license | Explicit owner selection; root LICENSE and Cargo manifests |
| D22 | This request implements Phase 0 only; first usable milestone remains Phase 0–1 | Explicit request scope; do not start Phase 1 implicitly |

## Accepted Phase 1 decisions

The user explicitly requested Phase 1 and approved the proposed defaults before implementation. D22 remains the historical scope of the earlier bootstrap request; it does not constrain this delivery. [Phase 1 evidence](phase1.md) resolves S01/S02 on the tested Linux filesystem.

| ID | Accepted choice | Rationale / evidence |
|---|---|---|
| D10 | XDG config (HOME fallback) outside the library; selected canonical root and root-keyed cooperative locks; no cache until needed | Shared selection and app-state deletion/reselection tests; Phase 1 has no derived DB |
| D11 | Recursive hidden-inclusive lowercase `.md` discovery; no symlink following; no hard-link writes; portable UTF-8 mutation paths | V02/V07 tests; unsupported entries remain unchanged and diagnosed |
| D12 | Validate with pinned `serde_yaml_ng` and `yaml-rust2`; patch owned fields, never reserialize imported YAML | S01 fixture tests; unsupported YAML/resource limits explicitly described in Phase 1 notes |
| D13 | Opaque SHA-256 revisions, cooperative nonblocking locks, same-directory staged/synced replacement, atomic Linux no-replace create/move | S02 stale/fault/process-kill tests; final external-writer/ancestor race remains documented |
| D14 | Every duplicate ID member ambiguous; no silent repair; explicit path inspection; incomplete identity maps block existing-note mutation | V05 plus malformed-metadata duplicate regression |
| D15 | `--library`, `--json`, `id:`/`path:` selectors; permanent deletion needs `--yes` or terminal confirmation | Stable envelope/exit codes and real executable/PTY tests; code 5 reserved for later doctor |
| D20 | No undo/history; confirmed permanent deletion of one note | Stale confirmation and deletion tests; empty directories preserved |
| D23 | Current request authorizes Phase 1 only, after completed Phase 0 | Stop after P1-07; no Phase 2+ scaffolding |
| D24 | Refuse replacements with ACLs/xattrs/special mode bits or changed owner/group; cap complete notes at 16 MiB | Do not silently broaden access or commit notes the reader cannot reopen; actual ACL/group/size tests |

## Accepted Phase 2 decisions

The user explicitly authorized Phase 2 and approved D16 before implementation. Earlier D22/D23 request scopes are historical.

| ID | Accepted choice | Evidence |
|---|---|---|
| D16 | Literal tokens by default, explicit phrase/prefix modes, exact case-sensitive tags and component-aware folders | Core/CLI hostile and Unicode query fixtures; SQLite unicode61 tokenization |
| D25 | Phase 2 only; bundled SQLite/FTS5, transactional cache, root-lock-scoped connections with DELETE journaling | Cache deletion/corruption and concurrent process tests; [Phase 2](phase2.md) |
| S05 baseline | Record real 1k/50k measurements; 100 ms search p95 remains provisional, not a release guarantee | [Raw baseline and environment](phase2-baseline.json); tmpfs and warm OS-cache limits explicit |

## Proposed implementation defaults for later phases

| ID | Proposal | Why / decision gate |
|---|---|---|
| D10 | OS-native config/cache directories outside library; config selects root, cache keyed by canonical root | No synced database/lock files; moving root may rebuild; resolve in P1-01 |
| D11 | Lowercase `.md` regular files only; do not follow symlinks; reject symlink root and mutation paths; refuse writes to hard-linked notes | Tight portable security boundary; non-UTF-8 names diagnosed and not mutated; resolve in P1-01 |
| D12 | Preserve body bytes, BOM/newlines, unrelated frontmatter text where practical; malformed YAML, duplicate keys, invalid IDs/tags are non-mutating diagnostics | General YAML serialization can destroy comments or values; resolve parser strategy in P1-02 |
| D13 | Hash-based revision preconditions for updates/moves/deletes plus local app-process serialization | Protect against detectable stale writes without storing revision metadata in notes; resolve in P1-03 |
| D14 | Duplicate ID makes every member ambiguous; exclude the whole group from ID lookup/search until manually repaired | Never let scan order choose a winner; path-based diagnostic viewing stays available; resolve in P1-04 |
| D15 | CLI human output plus stable `--json`; explicit `id:` / `path:` selectors disambiguate; destructive CLI delete requires `--yes` or terminal confirmation | Scriptability and deliberate destruction; resolve in P1-06 |
| D16 | Literal text search by default, explicit phrase/prefix modes, exact case-sensitive tag filter, component-aware folder filter | Avoid exposing raw FTS syntax accidentally; resolve in P2-02 |
| D17 | Source editor and sanitized preview first; rich mode only if round-trip spike passes | Meet v1 without sacrificing Markdown preservation; resolve in P5-01 |
| D19 | Dirty conflict resolution offers reload (explicit discard) or save local buffer as a new note with new ID; no blind force-overwrite | Preserve both versions without introducing history; resolve in P6-01 |
| D20 | No delete undo/history in v1; delete is a confirmed permanent filesystem removal | Source requires deletion but excludes history; make limitation visible; resolve in P1-06 |

## Evidence-required spikes

| ID | Question | Evidence / bound | Gate |
|---|---|---|---|
| S01 | Can chosen parser safely edit only app-owned YAML fields? | Fixtures with unknown nested values, comments, anchors/aliases, duplicate keys, delimiters, CRLF/BOM, malformed input. Select maintained parsing/token tooling; explicitly refuse unrepresentable mutations rather than round-trip lossy YAML. One focused spike, not a parser framework. | P1-02 |
| S02 | Which safe-replace/no-clobber primitives work on declared targets? | Actual temp-directory fault tests for flush/replace, parent durability, collision, symlink/hard-link checks, move interruption; document unsupported filesystems and external-writer race window. | P1-03 |
| S03 | Which editor preserves the required Markdown? | Compare source/preview baseline with Milkdown/ProseMirror using untouched round trips and edited fixtures; reject silent loss. No rich mode if evidence fails. | P5-01 |
| S04 | Watcher and shared cache under real filesystem behavior? | Multiple processes, atomic-save external editor, directory moves, overflow recovery, permissions, sleeping/reopening; verify on declared OS/filesystem matrix. | P3-01 through P3-03 |
| S05 | What performance thresholds are reproducible? | Fixed hardware, fixture distribution, cold/warm cache definitions; adopt numeric targets before optimization, report distributions, never invent benchmark results. | P2-04 and P7-03 |

## Risks to keep visible

1. Atomic replacement prevents partial files but is not an atomic compare-and-swap against arbitrary external editors. Hash checks narrow, not eliminate, the final check/write race. Cooperative locks protect Foglio processes only.
2. Import adoption itself changes user files. Read-only/malformed/unstable input must be reported without destructive fallback. Adoption must recheck bytes before commit.
3. Two synced devices may independently adopt the same missing-ID file. v1 has no distributed identity authority; establish IDs before initial sync where practical and preserve/report divergent results.
4. Sync conflict copies often duplicate IDs. Treating them as ordinary Markdown still requires ambiguity handling, not silent regeneration.
5. Filesystem and SQLite cannot share a transaction. Durable file commit must not be reported as if nothing happened merely because indexing failed.
6. Metadata-only warm scans can miss same-size/same-mtime edits while offline. Running watcher paths require content checks; full rescan/reindex is the definitive recovery. Document freshness limits rather than promising impossible cheap certainty.
7. Mounts/network/cloud placeholder files may not support tested atomic/durability semantics. Detect/report unsupported behavior; do not claim all remote filesystems safe.

Decisions may be amended only with explicit rationale and updated affected specs/tests/tasks. Established choices are not reopened just to explore alternative stacks.
