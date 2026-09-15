# Vim source editing and relative line numbers

**Status: product questions resolved by the owner; implementation not started.** This branch contains specifications and handoff tasks only. Open a PR later; do not open one as part of this handoff. Baseline inspected: `e692083` on `origin/main`.

## Outcome and scope

Let users edit Markdown source with familiar Vim motions and Normal, Insert, Visual and Command-line modes, while retaining ordinary editing as the default. A user must be able to enable or disable Vim without losing their buffer or undo history. Deliver configurable relative line numbers as a separate follow-up, usable with either keybinding style.

“Visual” means Vim selection, not Foglio's rendered **Preview**. Source/Preview remains a separate view choice. This is embedded Vim keybinding compatibility, not a full Vim/Neovim runtime: no shell, plugins, `.vimrc`, arbitrary filesystem commands, terminals or LSP in this scope.

The owner decisions at the end settle compatibility, shortcut ownership, command-line scope and relative-number semantics. Remaining technical choices are implementation guidance; the dependency spike still gates the editor-engine decision. This handoff remains documentation-only, not a request to implement now.

## Current implementation and constraints

- `apps/desktop/src/app.ts` owns a plain `textarea`, `sourceWrap`, absolute-number gutter, source/preview switching, formatting toolbar and document-level shortcuts. `toggleLineNumbers()` is currently session-only; `updateGutter()` responds to edits/renders, not all caret movements. The source does not soft-wrap.
- `apps/desktop/src/source-history.ts` owns per-note undo/redo, raw-body snapshots, selection, typing/composition grouping and programmatic replacements. Saves and view switches preserve history; a newly installed note resets it. Browser document-wide undo was deliberately avoided.
- `apps/desktop/src/editor.ts` owns raw body, newline preservation, revision-guarded debounced autosave, readback verification and conflict/missing-file handling. The source surface edits the **body**, not frontmatter. Keep this ownership and the existing backend safety contracts.
- `apps/desktop/src/format.ts` and the link helper in `markdown.ts` operate on source ranges. They must participate in the same history as keyboard edits.
- `apps/desktop/src/api.ts` and `apps/desktop/src-tauri/src/lib.rs` expose appearance preferences. The backend persists appearance separately in `desktop-appearance.json`; shared library selection is separate. There is no editor-settings API yet.
- `apps/desktop/package.json` currently has no editor engine or Vim dependency. Adding one is an explicit architectural change, not activating an existing option.
- Existing acceptance harnesses `tests/acceptance/phase5.py`, `phase6.py` and `phase7_keyboard.py` exercise real Tauri/WebKit editing, saves, conflicts and keyboard behavior. Textarea-specific assertions will need careful adaptation, not deletion of their behavioral coverage.

## User experience

### Choosing keybindings

Add a keyboard-accessible **Editor settings** control near the existing Appearance control. Its dialog initially offers **Keybindings: Standard / Vim**. Do not rename Appearance or introduce a general settings redesign for this feature. Later add the line-number settings in the same dialog.

- Default: **Standard** for existing and new installations.
- Persist per desktop profile, across libraries and restarts, outside Markdown, library config and the derived index.
- Apply on explicit confirmation after successful persistence. A failed settings write keeps the previous preference active and reports the failure; it never drops text or reports a saved preference that was not saved.
- Switching is a live reconfiguration, not note reopening or editor reconstruction. Preserve text, raw newlines, selection, scroll, save/conflict state and undo/redo. Enabling Vim starts in Normal at the active caret; disabling cancels only pending Vim commands/prompts and returns ordinary editing. Retain a Visual selection as a standard selection when disabling.
- Do not switch in the middle of IME composition or a protected note transition; defer until composition finishes or disable Apply while busy, with a visible explanation.
- Settings remain reachable without knowing a Vim command. No new global toggle chord initially: avoid inventing another collision.
- No note open: settings are still available. New notes/source entry start in Normal when Vim is enabled. Standard editing must not acquire modal behavior.

### Mode and focus behavior

Show a compact, accessible source-only mode indicator: **NORMAL**, **INSERT**, **VISUAL**, **VISUAL LINE**, and **COMMAND** (search prompts may show **SEARCH**). Distinguish modes by text and cursor shape, not color alone. Announce mode changes politely, not every motion.

`i` enters Insert; `Esc` returns to Normal and cancels pending operators/counts. `v` and `V` select characters and logical lines; `:` opens a command line; `/` and `?` open forward/backward in-buffer search. `Esc` cancels a prompt without modifying source, restoring source focus. Unknown Ex commands show a visible error and retain text. Command/search inputs have labels and keyboard focus behavior; a failed save must not erase the error via a misleading success message.

Switching to Preview, opening a modal, or leaving the source cancels pending command prefixes and exits Visual/Insert to Normal without undoing committed edits. Returning to Source keeps caret/scroll/history but starts in Normal. Autosave rerenders must **not** reset mode or cursor. Switching notes clears pending commands, search decorations and per-note history; no command started on one note may execute on another.

Tab/Shift+Tab retain an escape path through the application; do not introduce a keyboard trap. Native IME/composition, dead keys, AltGr and paste must work in Insert and Standard. Only the focused source surface and its own prompt receive Vim input; library search, metadata dialogs and Preview remain ordinary controls.

### Minimum compatibility contract

The following are required, tested behavior, not a promise of full Vim parity. Additional engine behavior must be audited for host actions and documented; do not silently ship unsafe Ex defaults.

| Area | Required keys/behavior |
|---|---|
| Modes | `Esc`, `i`, `a`, `I`, `A`, `o`, `O`, `v`, `V`, `:` |
| Motions | `h j k l`, `w b e`, `0 ^ $`, `gg`, `G`, `{ }`, `f{char}`, `t{char}`, `;`, `,`, `%` |
| Counts/operators | Composable counts and `d`, `c`, `y` with motions; `3j`, `2dw`, `d2w`, `dd`, `cc`, `yy`, `D`, `C`, `x`, `r{char}` |
| Text objects | `iw`, `aw`, `i"`, `a"`, `i(`, `a(` with delete/change/yank and Visual selection |
| Paste/register | Internal unnamed register with `p`/`P`; never require clipboard permission for ordinary yank/paste |
| History/repeat | `u`, `Ctrl+r`, `.`; one operator is one undo step, and an Insert session is a coherent undo unit |
| Selection | Characterwise/linewise selection with motion/counts and `d`, `c`, `y`; correct linewise paste and last-line behavior |
| Search | `/`, `?`, Enter, `n`, `N`, `*`, `#`; within current body only, with visible no-match feedback |
| Paging | `Ctrl+d`, `Ctrl+u`, `Ctrl+f`, `Ctrl+b`, `Ctrl+e`, `Ctrl+y` |
| Command line | `:w` / `:write` flush existing guarded autosave; `:<line>` jumps to a body-relative logical line; `:noh` / `:nohlsearch` clear highlighting |

`:w` accepts no path argument and must await `Editor.flush()`/verification. Autosave remains enabled; `:w` is an explicit flush, not a new write path. Failed/conflicted/missing saves retain the buffer and existing recovery UI. Do not implement `:w!` as force-overwrite.

Initially reject host-sensitive commands such as `:q`, `:q!`, `:wq`, `:x`, `:e`, `:saveas`, `:r`, `:!` and `:source` with a clear unsupported-command message. No command may bypass the existing close/navigation guards. Substitution, macros, named/system clipboard registers, marks, Visual block and custom mappings are not first-delivery acceptance requirements; decide whether to expose supported engine-native extras only after the spike documents their safety and limitations. This bounded contract must be visible in keyboard help; do not advertise “all Vim commands.”

### Shortcut precedence

A single dispatcher owns precedence; avoid both the Vim extension and the existing document listener handling one event.

1. Open application dialogs/menus own their input. Composition/dead-key input is not interpreted as a Vim command.
2. When the source has focus and Vim is enabled, the Vim keymap owns its supported Ctrl chords in Normal/Visual (including `Ctrl+f/b/e/u/d/r/y`), even when Foglio previously used them globally.
3. In Vim Insert, reserve Ctrl combinations recognized by the engine for Vim too; explicitly test `Ctrl+w` and `Ctrl+u` insertion editing and audit further collisions. Application actions remain available via controls. Do not use document handlers as an accidental fallback for a partially consumed Vim sequence.
4. Reserve `Ctrl+s` for guarded Save in Vim on Linux. On macOS, Command-based application shortcuts remain application actions; Vim's Control-based bindings remain distinct. Vim mode help must show the actual platform-specific precedence.
5. Outside the focused Vim surface, preserve existing application shortcuts. Standard mode retains its existing keymap.

On Linux, `Ctrl+e` in Vim scrolls instead of toggling Preview, `Ctrl+f` pages instead of focusing library search, and `Ctrl+b` pages instead of inserting bold. Source/Preview buttons, search focus via keyboard traversal, formatting controls and Editor settings remain reachable. Toolbar actions are one source transaction and participate in Vim undo; exit Visual/Insert to Normal after a toolbar mutation to avoid retaining a stale pending command. This precedence is approved in Q2; document it rather than quietly breaking either keymap.

## Architecture recommendation and spike gate

**Preferred candidate: CodeMirror 6 with `@replit/codemirror-vim`, using one source surface for Standard and Vim.** CM6 models plain text, not a Markdown AST serializer. Toggle its Vim extension dynamically; do not keep separate textarea and CodeMirror buffers or build a home-grown Vim parser.

Alternatives considered:

- A custom textarea motion layer preserves today's DOM but grows into a modal parser, register system, selection renderer and repeat engine. Too much correctness surface for the requested compatibility.
- A native Vim/Neovim process adds process/RPC/lifecycle, packaging and security complexity disproportionate to a source editor.
- CodeMirror is the recommended reuse path, but it replaces the source surface even for Standard users. Dependency footprint, native WebKit behavior and byte/history preservation are mandatory gates, not assumed benefits.

References reviewed for this proposal: [CM6 reference](https://codemirror.net/docs/ref/) (state, transactions, compartments, history and gutters), [Vim extension repository](https://github.com/replit/codemirror-vim). Its documented CM6 integration is evidence of a candidate, **not evidence that Foglio integration works**. No dependency was installed or runtime spike executed for this documentation branch.

See the [system design and C4 views](vim-source-editing-architecture.md) for ownership, transaction flows, lifecycle invariants and spike deliverables. These describe a target architecture, not existing modules.

### VM-01 spike must prove

1. Pin compatible dependency versions and inspect license, transitive dependencies, extension precedence, mode events, command hooks, dynamic enable/disable and teardown. Record exact versions and source/API findings rather than guessing imports from this proposal.
2. Exercise real Linux Tauri/WebKit input: Insert, Visual, count/operator, `u`, redo, dot repeat, command/search prompts, composition and live keymap switching. Measure bundle delta, source-open and input responsiveness against baseline on recorded representative documents; report measurements before agreeing a budget.
3. Establish **one authoritative history**, shared by engine Vim commands, Standard shortcuts and toolbar actions. The existing textarea `SourceHistory` cannot run alongside CM history. Prefer CM history plus transaction-linked raw-body undo metadata; prove it using real undo after autosave, formatting, mode switches, redo and mixed newlines. If the engine requires a second competing history, stop and revisit the design.
4. Prove a lossless transaction bridge. CM normalized text is not canonical Markdown. Keep raw body in `Editor`; map precise transaction ranges to raw offsets and apply separated changes without rewriting unchanged spans. Feeding one whole-document diff to `preserveNewlines()` can normalize untouched lines between separated edits. Undo/redo must restore exact raw bytes, not reconstruct deleted mixed newlines from normalized text. Verify history metadata remains associated with the correct transaction even after autosave.
5. Preserve the backend/frontmatter boundary, dirty-buffer conflict recovery, acknowledgement readback and close guards. Initial load, cursor moves, mode switches and settings changes never call save solely because the view changed. Prevent listener feedback loops when installing a snapshot.
6. Audit all host-sensitive Ex handlers; unsupported commands cannot silently write, open paths, launch URLs/processes, quit or discard. Search UI must render user text safely under production CSP.
7. Render with actual theme CSS in native WebKit. Preserve no-soft-wrap behavior, sticky/aligned gutter, horizontal scrolling, selection visibility and legibility at small sizes in light/dark themes. Use the engine's scroll ownership rather than copying textarea fitting mechanics blindly. Check focus escape and accessible naming.

Write evidence in proposed `docs/vim-editor-spike.md`. If any preservation/history/security gate fails, mark VM-02 blocked and record alternatives; do not ship a reduced custom engine as an unapproved fallback.

### Integration boundaries (proposed new modules)

- Introduce `apps/desktop/src/source-editor.ts` as the owned source-surface adapter: install snapshot, set keybindings/read-only state, focus, selection, transactions, history, mode changes and destruction. Define its actual interface during VM-01; keep dependency APIs out of most of `app.ts`.
- Introduce `apps/desktop/src/editor-preferences.ts` for typed preference values. Mirror Rust enums in `src-tauri/src/lib.rs` and typed IPC in `api.ts`.
- Proposed `desktop-editor.json`, alongside appearance settings, stores `keybindings: standard | vim`. Follow existing safe filesystem read/write conventions, not browser localStorage or the notes index. Missing file/field uses Standard; malformed/unsupported values are reported without rewriting the file. Treat writes atomically and prevent overlapping UI requests from persisting stale values.
- Later extend this same file with `lineNumbersVisible: boolean` and `lineNumberStyle: absolute | relative`. Older valid settings without these fields use visible/absolute. Preserve all supported fields when changing one preference. Do not store cursor, transient mode, search text or registers as settings.
- Keep `Editor` as save/conflict owner; add only the lossless transaction/history boundary necessary for CM6. No database migration, CLI change, Markdown reserialization or backend write bypass.
- Remove obsolete textarea history/gutter code only after equivalent tests pass. Update selectors in native tests to semantic source-editor identifiers and real keyboard actions; do not replace native coverage with fabricated input events.

## Follow-up: relative line-number column

Separate delivery **RN-01/RN-02**, after Vim ships; not a prerequisite for Vim and not automatically enabled by it.

Editor settings offer **Show line numbers** and **Numbering: Absolute / Relative**. This separation retains the chosen style while hiding the gutter. Keep defaults visible/absolute. Existing Ctrl/Cmd+L toggles visibility and, once this feature lands, persists that visibility without cycling styles; revert/report on persistence failure.

- Absolute: each logical body's line number.
- Relative: distance on other lines, actual absolute number on the current line (Vim `number` + `relativenumber` semantics, often called hybrid). The UI calls this **Relative**; there is no separate Hybrid option and no current-line-zero option in scope.
- Owner acceptance example: a four-line document with the cursor on line 3 displays **`2, 1, 3, 1`** from top to bottom. For one-based line `n` and active line `c`, render `n` when `n === c`, otherwise `abs(n - c)`.
- Count body lines, excluding frontmatter, matching today's gutter. Empty body is one line; a trailing newline introduces the final empty line. No soft-wrap change in this feature.
- Use the active selection head, not always the smaller selection offset, including backward Visual/mouse selections. Use the primary selection if the engine exposes more than one; no multi-cursor product feature is added.
- Update on selection-only transactions (Vim motions, arrows, mouse, search jumps), edits, undo/redo, note installation and external clean reload. Gutter rendering must not dirty the document, schedule saves, create history entries or steal focus. On blur retain the last active line until a new selection is established.
- Keep stable gutter width for the document's maximum absolute line label, even near its edges, so cursor motion cannot shift text horizontally. Hidden means no gutter space. Scrolling must keep labels aligned and support virtualized rendering rather than constructing all line nodes on every motion.
- Formatting/display style does not depend on the keymap or Insert vs Normal. Disabling Vim retains the user's numbering choice.

## Delivery plan and acceptance

VM-00 is **complete** on the owner’s explicit answers; all implementation and verification tasks remain **open**. Each future implementation PR must link this specification, report phase status, exact tests and native evidence, and explicitly name unmet gates. Use focused stacked PRs for dependent phases; rebase rather than merging main.

| Task | Depends on | Deliverable | Exit gate |
|---|---|---|---|
| [x] VM-00 | — | Owner accepted core scope, Vim shortcut priority, bounded Ex commands and absolute-current relative numbering; see decisions below | Product questions resolved; technical spike still required |
| [ ] VM-01 | VM-00 | Disposable candidate spike and `docs/vim-editor-spike.md` | Every spike gate above demonstrated or implementation blocked |
| [ ] VM-02 | VM-01 | Shared source adapter, lossless transactions and unified history, initially Standard only | A01, A03, A06–A09; existing editor behaviors preserved |
| [ ] VM-03 | VM-02 | Persistent preference/IPC, settings UI, Vim keymap, indicator, prompts, dispatcher and bounded Ex hooks | A01–A10; keyboard help documents compatibility/precedence |
| [ ] VM-04 | VM-03 | Native Vim acceptance, docs, dependency/security review | Full gates below, review of native rendering and accurate limitations |
| [ ] RN-01 | VM-04 | Number style/visibility persistence and selection-driven gutter | R01–R04, works in both keymaps |
| [ ] RN-02 | RN-01 | Native gutter acceptance and help/docs | R01–R05 and relevant full regression gates |

### Vim acceptance scenarios

| ID | Pass condition and evidence |
|---|---|
| A01 | Existing config opens Standard; enable/disable using keyboard and pointer; preference survives restart/library switch. Missing/invalid config and failed writes tested without note/config loss. |
| A02 | Every required compatibility-table row has deterministic fixtures; native keyboard sequences prove `3j`, `d2w`, `ciw`, `V` + motion + yank/paste, Insert, `Esc`, undo/redo and dot repeat. Assert exact text, caret, selection and indicated mode, not just event receipt. |
| A03 | Toggle Standard/Vim while dirty, after autosave, with selection and redo available; text/raw bytes, history and scroll survive. Disable with pending operator/prompt cancels it. No stale command executes on another note or after Preview/modal focus. Autosave does not reset mode. |
| A04 | Real command input: `:w` succeeds only after readback; conflict/write failure retains buffer; `:999` clamps to final line; unsupported/force/path commands cannot overwrite, quit or discard. Search Enter/Esc/n/N and no-match behavior work without dirtying source. |
| A05 | Linux Vim Ctrl paging is not library search, formatting or Source/Preview; Save still works. Standard and non-source controls keep application shortcuts. macOS Command-vs-Control dispatch has unit coverage and native evidence before claiming macOS Vim qualification. Prompts/modals never trigger hidden source edits. |
| A06 | LF, CRLF, lone CR, mixed-newline, BOM/frontmatter, Unicode, emoji, combining marks, tabs, empty and final-empty-line fixtures preserve unchanged bytes. Separated edits and undo/redo after autosave restore exact expected raw body; opening/toggling/navigating without editing causes zero note writes. Never split a surrogate pair through a motion/edit. |
| A07 | Single history for keyboard/toolbar/commands; undo never modifies library search or a previously open note. New snapshot resets history; saves/view/keymap switches do not. IME is one coherent edit and no command steals composition. |
| A08 | Existing delayed-acknowledgement, dirty external edit/delete, retry, save-copy and close/navigation acceptance still passes through the new surface. No implicit force save, resurrection or alternate filesystem path. |
| A09 | Native themes, narrow window, long lines and long document render with correct scrolling/gutter/selection; no keyboard trap. Mode indicator is understandable without color. Record accessibility/native IME manual limitations separately from DOM assertions. |
| A10 | Malicious-looking note/search/prompt text is inert under production CSP. Teardown/reopen does not duplicate key handlers. Candidate dependency/version/license and performance measurements are recorded, with agreed budgets or explicit blockers. |

### Relative-number acceptance scenarios

| ID | Pass condition and evidence |
|---|---|
| R01 | Deterministic pure label tests cover every style at first/middle/last lines, empty document and trailing newline; use the style definitions above as the oracle. Pin the owner example: four lines, active line 3, Relative labels `2, 1, 3, 1`; current line never displays zero. |
| R02 | Arrow/Vim/mouse/backward-selection/search/undo moves update labels without text edits, dirty state, history entries or saves. Active head, not anchor, determines the current line. |
| R03 | Styles/visibility persist across restart and libraries, old settings gain visible/absolute defaults, hide/show retains style, and Vim toggling cannot alter the preference. Persistence failure remains visible and does not claim success. |
| R04 | Native scrolling and long lines stay aligned; gutter width is stable across cursor movement and digit boundaries; hidden gutter consumes no space. Standard and Vim both pass. |
| R05 | Large-document caret motion uses bounded/viewport rendering; record comparable before/after measurements. No fabricated performance guarantee, no full-document DOM rebuild per motion. |

### Required implementation checks (not run for this docs-only proposal)

From `apps/desktop`:

```sh
npm ci
npm run typecheck
npm test
npm run build
```

From the repository root:

```sh
cargo fmt --all -- --check
cargo test --locked --workspace --all-features
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
```

Build the native test executable **after** Cargo test commands, from `apps/desktop` with `npm run tauri -- build --debug --no-bundle`. From the repository root, run each harness with `FOGLIO_DESKTOP_BINARY=target/debug/foglio-desktop` and `TAURI_DRIVER=$HOME/.cargo/bin/tauri-driver` in its environment:

- Existing `tests/acceptance/phase5.py`, `phase6.py`, `phase7_keyboard.py` (and `phase4.py` for read-only regression).
- Proposed new `tests/acceptance/vim_editor.py` for VM-04 and `tests/acceptance/relative_line_numbers.py` for RN-02; these files do not exist yet. Reuse existing isolation/driver helpers and temporary fixture libraries, not the user's notes.
- Native tests must send real WebDriver keyboard actions, inspect on-disk bytes and exercise the production webview. Unit tests/synthetic events alone do not qualify native Vim editing or IME.
- Report exact test commands, platform/engine/package versions and actual outcomes. A Linux pass is not native macOS verification. Adapt CI to run the new harnesses before closing tasks.

## Owner decisions (resolved)

1. **Q1 — Core scope accepted.** Core motions/operators are sufficient initially; macros, Visual block, named/system clipboard registers and substitution are not first-delivery requirements.
2. **Q2 — Vim shortcut priority accepted.** Vim owns conflicting Control chords while its source is focused, as specified above.
3. **Q3 — Bounded command line accepted.** Start with guarded `:w`, line jumps and clearing search highlighting; reject the listed quit/force-write/path/shell commands.
4. **Q4 — Absolute current line required.** Relative numbering shows distances on other lines and the absolute number on the cursor line. The owner example is `2,1,3,1` with the cursor on line 3. This supersedes the earlier three-style proposal; ship only Absolute and Relative choices.

The owner also requested system-design/architecture/C4 guidance for the implementing agent; see the linked architecture document. Standard default, profile-wide persistence and separate delivery remain the plan’s defaults. No product question above remains open. Dependency selection, raw-history integration and performance qualification remain evidence gates, not approved implementation results.

## Remote-agent handoff

Branch: `docs/vim-source-editing`. Documentation-only worktree: `../foglio-vim-source-editing`. No feature implementation, dependencies, runtime artifacts or PR belong in this delivery.

Fetch this branch and read this document plus [tasks](../tasks.md), [Phase 5 preservation evidence](../phase5.md), [Phase 6 recovery evidence](../phase6.md) and the current source before starting. If main has advanced, rebase the specifications and reconcile affected symbols/behavior. Owner answers are recorded above. Start with VM-01, and carry VM-02 onward in dependent branches/PRs only after the spike gate. Keep RN tasks durable and open until independently verified; completing Vim does not close relative numbering.
