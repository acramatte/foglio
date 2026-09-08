//! Linux local-filesystem Markdown domain with a disposable search index.
pub mod events;
pub mod filesystem;
pub mod frontmatter;
pub mod index;
pub mod library;
pub mod search;
pub mod watcher;
pub use filesystem::LibraryRelativePath;
pub use frontmatter::{Document, NoteId};
pub use library::{Library, Report};
use serde::Serialize;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Io,
    Index,
    Metadata,
    Encoding,
    Path,
    Unsupported,
    Permission,
    Conflict,
    Exists,
    Busy,
    Config,
    Committed,
    Ambiguous,
    Skipped,
    MissingId,
    NotFound,
    Incomplete,
    Usage,
    Cancelled,
}
impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Io => "io",
            Self::Index => "index",
            Self::Metadata => "metadata",
            Self::Encoding => "encoding",
            Self::Path => "path",
            Self::Unsupported => "unsupported",
            Self::Permission => "permission",
            Self::Conflict => "conflict",
            Self::Exists => "exists",
            Self::Busy => "busy",
            Self::Config => "config",
            Self::Committed => "committed",
            Self::Ambiguous => "ambiguous",
            Self::Skipped => "skipped",
            Self::MissingId => "missing_id",
            Self::NotFound => "not_found",
            Self::Incomplete => "incomplete",
            Self::Usage => "usage",
            Self::Cancelled => "cancelled",
        }
    }
}
impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
#[derive(Debug, Clone, Serialize)]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
    /// Present when the mutation happened, even if durability is uncertain.
    pub commit: Option<filesystem::Commit>,
}
impl Error {
    pub fn new(code: ErrorCode, message: impl ToString) -> Self {
        Self {
            code,
            message: message.to_string(),
            commit: None,
        }
    }
    pub fn committed(message: &str, commit: filesystem::Commit) -> Self {
        Self {
            code: ErrorCode::Committed,
            message: message.into(),
            commit: Some(commit),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::new(ErrorCode::Io, e)
    }
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Revision(String);
impl fmt::Display for Revision {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
pub fn revision(bytes: &[u8]) -> Revision {
    use sha2::{Digest, Sha256};
    Revision(format!("{:x}", Sha256::digest(bytes)))
}
