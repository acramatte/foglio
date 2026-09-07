# Foglio

A local-first Markdown notes application with a reusable headless Rust core, a CLI, and a Tauri desktop client.

**Project status: planning only.** No application code, build configuration, or Git repository has been created. All implementation tasks are open.

Markdown files are authoritative. SQLite is a disposable index. External editing is supported; synchronization belongs to external filesystem tools.

## Project documents

- [Product specification](docs/specs/product.md): scope, requirements, release workflow, non-goals.
- [Technical specification](docs/specs/technical.md): file format, domain contracts, filesystem safety, indexing, watcher, CLI, desktop.
- [Implementation plan](docs/implementation-plan.md): vertical slices, dependencies, phase gates, delivery discipline.
- [Task backlog](docs/tasks.md): stable task IDs, deliverables, dependencies, acceptance criteria.
- [Verification specification](docs/specs/verification.md): test scenarios, traceability, release evidence.
- [Decision register](docs/decisions.md): established decisions, proposed defaults, implementation spikes.

Source: the user-supplied **Headless Markdown Notes — v1 Specification & Implementation Plan.md**, sections 1–44, also located at `/home/alexis/Downloads/Headless Markdown Notes — v1 Specification & Implementation Plan.md`. The original brief was not copied into this repository.

## How to use this plan

1. Review the proposed decisions in the [decision register](docs/decisions.md) before their dependent tasks.
2. Begin with **P0-01**, then implement Phase 0 and Phase 1 only for the first delivery.
3. Keep subsequent phases as backlog, not scaffolding.
4. Mark tasks complete only after their acceptance checks and phase gate actually pass. Record commands, results, and review/PR links alongside the task.

`Foglio` is the project name derived from this directory. The plan retains `notes-core`, `notes-cli`, and the `notes` executable from the source brief; changing these names is an explicit pending decision, not an implicit rename.

No daemon, built-in sync, encryption, attachments, AI, or collaboration is part of v1.
