# External transfer and synchronization

Foglio has no synchronization engine, account, daemon, merge protocol or remote
API. Markdown files are authoritative; use an external filesystem tool to move
or synchronize a library. **Actual second-device release acceptance is still
blocked.** A local copy test does not qualify any provider or another device.

## What to transfer

Transfer the library directory, including ordinary Markdown and any metadata
already in those files. Do **not** transfer Foglio's application configuration,
SQLite database/WAL/SHM, locks or cache. Default application state lives outside
the library under `$XDG_CONFIG_HOME/foglio` (normally `~/.config/foglio`) and
`$XDG_CACHE_HOME/foglio` (normally `~/.cache/foglio`). Each device selects its own
absolute root and reconstructs its own derived index.

On each device, select the existing folder in the desktop, or use
`notes init /absolute/path/to/library`. Selection does not adopt/rewrite the
Markdown or insert IDs. Avoid configuring a tool to silently resolve concurrent
edits by overwriting one side. Retain conflict copies as independent `.md`
files, with different paths; equal content is not deduplicated.

## Conservative transfer procedure

1. Save and close Foglio and external editors on the source device. Resolve or
   copy retained dirty buffers before closing. Foglio does not have crash drafts.
2. Back up the library independently. Record relative paths and content hashes.
3. Preview the external tool's transfer plan. Transfer only the library to a
   separate destination using the chosen tool's documented safe procedure.
   Do not use destructive mirror/delete flags without reviewing their scope.
4. Compare the destination paths and byte hashes against the source manifest.
5. On the destination device, select the transferred library. Run
   `notes --library /absolute/destination reindex`, then
   `notes --library /absolute/destination doctor` and test search/navigation.
6. Test a deliberate separately named identical conflict copy. Both paths must
   remain accessible, including after closing the app and rebuilding its cache.

These are operational instructions, not a claim that a provider or device has
been tested. Before a release claim, record the tool/version, both OSes,
filesystem types, manifest comparison, conflict-copy result, and packaged
application behavior on the actual second device.

## Concurrent editing limits

A clean selected note reloads observed external changes. A dirty or saving
buffer pauses on an observed conflicting revision; explicitly reload/discard or
save local content to a different unoccupied path. External moves/deletions do
not retarget the selection by content hash and autosave must not resurrect a
missing source. The local cooperative lock does not coordinate different devices
or arbitrary external editors. A final external-write race remains between
revision checks and replacement; this is not a distributed compare-and-swap.

Remote/network-mounted filesystems, macOS, Windows, provider-specific conflict
naming, actual suspend/resume and simultaneous cross-device editing are not
qualified merely because local Linux tests pass. Do not use an adversarially
shared library directory; see [filesystem safety](phase1.md).
