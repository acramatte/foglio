//! Body-free, generation-stamped invalidations. Refetch disk before mutation.
use crate::{Revision, library::Diagnostic};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WatchedNote {
    pub path: String,
    pub revision: Revision,
}
#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub generation: u64,
    pub kind: EventKind,
}
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventKind {
    NoteCreated {
        note: WatchedNote,
    },
    NoteChanged {
        note: WatchedNote,
        previous_revision: Revision,
    },
    NoteDeleted {
        note: WatchedNote,
    },
    DiagnosticsChanged {
        diagnostics: Vec<Diagnostic>,
    },
    IndexStateChanged {
        degraded: bool,
        error: Option<String>,
    },
    RescanRequired {
        reason: String,
    },
}
