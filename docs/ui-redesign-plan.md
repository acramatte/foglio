# UI redesign plan

Status: implemented foundation and layout slice; external Omarchy palette integration remains deferred.
Base: `89fe9a8` (includes source overflow/sharpness, formatting bar, and word count).

## Scope and approach

Keep Tauri 2, the existing TypeScript DOM UI, textarea source editor, and marked/DOMPurify preview. Build one layout with Sober light and Omarchy-inspired dark appearances. No new UI framework, Markdown engine, or external Omarchy integration.

The shell is in `apps/desktop/src/app.ts`, styling in `apps/desktop/src/style.css`, and startup in `apps/desktop/src/main.ts`. Preserve the editor/session machinery in `editor.ts` and `source-history.ts` rather than rebuilding it when appearance changes.

## 1. Theme and preference foundation

- Add a small desktop theme module separating stored `system | light | dark`, resolved light/dark appearance, and built-in palette selection.
- Centralize semantic CSS tokens for surfaces, text, borders, selections, focus, status, links, and code. Keep dimensions and typography identical between appearances; source stays monospace. Do not add syntax highlighting just for this redesign.
- Validate Tauri's supported system-theme query/change events on native Linux and macOS before wiring startup and live following. If unavailable, resolve light. Explicit overrides ignore OS changes; returning to System re-resolves immediately.
- Load preference before showing themed app content; update root theme attributes in place, without recreating App, editor, or preview.
- Persist through the existing Rust config-directory/durable-write conventions, exposed through `apps/desktop/src-tauri/src/commands.rs` and `apps/desktop/src/api.ts`. Current `config.json` is only a serialized library path, shared with the CLI—not a general settings object. Add a separate desktop settings file in that directory rather than break its format. Surface read/write failures, and keep settings usable without an open library.
- Keep the palette source an internal boundary with only a built-in implementation; no provider picker or speculative plugin architecture.

## 2. Compact shared layout

In `app.ts` and `style.css`:

- Replace the large brand/tagline and permanent path form with understated chrome and a sidebar library selector. Move the existing path-entry/open mechanism into its menu/dialog; retain clear first-run onboarding.
- Retain folder/tag navigation, compact search/New note, and flat selectable note rows. Start near 180px navigation and 240px note-list widths, adapting to small windows.
- Use a document toolbar with relative path, Source/Preview, overflow, and Focus. Move rename/delete into overflow and tag actions beside metadata, retaining current mutation handlers and confirmations.
- Add Focus mode by hiding the first two panes without unmounting them. Keep an obvious exit control; ensure search shortcuts reveal the hidden search pane.
- Keep source formatting controls, line numbers, undo/redo, and real word count. Preserve the opaque source/preview content and separate scrolling wrappers from the current sharpness fix.
- Give preview a roughly 65–80-character reading width; allow source/code horizontal scrolling. Ellipsize long paths with an accessible full path rather than hiding essential actions.
- Consolidate real save/document status into one footer; expose diagnostics on demand, while errors, retry, and conflict recovery remain prominent and reachable.

## 3. Complete both appearances and interactions

- Add an accessible System / Light / Dark setting, defaulting to System.
- Apply both palettes to every existing surface, including dialogs, menus, search results, source, preview, links, code blocks, focus, disabled controls, and supported scrollbars/native controls.
- Use consistent symbolic icons with accessible names. Check keyboard operation, Escape/focus return, selected states, and contrast—not colour alone.
- Theme changes must retain library, selected note, dirty body, undo history, caret/selection, scroll positions, and Source/Preview state. Avoid content rerenders for colour-only changes.

## 4. Verify and hand off

- Add focused tests for preference defaults/resolution, live OS events, override persistence/restart, returning to System, unavailable system preference, and settings failures. Verify library selection and CLI config compatibility.
- Extend app tests for menu reachability, Focus restoration, and state preservation during theme changes; keep editor/file-safety regression coverage intact.
- Run desktop `npm test`, `npm run typecheck`, `npm run build`, Rust workspace tests, and formatting/lint checks. Extend the existing native acceptance harness under `tests/acceptance/` for the new controls rather than replacing real interactions with mocks.
- In an isolated temporary library, exercise opening, creating, searching, folder/tag filtering, move/rename, delete, source/preview, autosave, external changes, and conflicts. Check existing supported text/Markdown file handling and frontmatter preservation.
- Inspect native light/dark at the same window size, document, and editing mode, including small windows, long names/lines, code blocks, empty library, and a real error. Capture comparable screenshots and verify actual native system following on available platforms; document untested/platform-limited behavior explicitly.

## Deferred

Actual Omarchy palette discovery, file formats, notifications, and validation are a separate feature requiring official documentation and target-environment verification. No cloud, accounts, storage changes, or unrelated refactors.
