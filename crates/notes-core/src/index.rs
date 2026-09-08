//! Disposable derived generations. All connections live under the library lock.
use crate::{Document, Error, ErrorCode, Library, Result, filesystem, library::Diagnostic};
use rusqlite::{Connection, params};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Clone, Default, Serialize)]
pub struct IndexStatus {
    pub discovered_notes: usize,
    pub indexed_notes: usize,
    pub stale_notes: usize,
    pub ambiguous_notes: usize,
    pub parsed_notes: usize,
    pub reused_notes: usize,
    pub cache_rebuilt: bool,
    pub incomplete: bool,
    pub watcher_active: bool,
    pub diagnostics: Vec<Diagnostic>,
}
#[derive(Clone, PartialEq)]
struct Cached {
    path: String,
    id: String,
    title: String,
    body: String,
    tags_json: String,
    hash: String,
    fingerprint: String,
    stale: bool,
}
pub(crate) fn db_error(e: rusqlite::Error) -> Error {
    let code = match &e {
        rusqlite::Error::SqliteFailure(e, _)
            if matches!(
                e.code,
                rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked
            ) =>
        {
            ErrorCode::Busy
        }
        _ => ErrorCode::Index,
    };
    Error::new(code, e)
}
fn corrupt(e: &rusqlite::Error) -> bool {
    matches!(e, rusqlite::Error::SqliteFailure(e, _) if matches!(e.code, rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase))
}
fn connect(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.busy_timeout(Duration::from_millis(250))?;
    conn.execute_batch("PRAGMA foreign_keys=ON; PRAGMA trusted_schema=OFF; PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL;")?;
    // Exercise the compiled extension rather than trusting a Cargo feature name.
    conn.execute_batch(
        "CREATE VIRTUAL TABLE temp.fts_probe USING fts5(text); DROP TABLE temp.fts_probe;",
    )?;
    Ok(conn)
}
fn secure_cache(path: &Path) -> Result<()> {
    filesystem::check_chain(path)?;
    for suffix in ["", "-journal", "-wal", "-shm"] {
        let p = PathBuf::from(format!("{}{suffix}", path.display()));
        match fs::symlink_metadata(&p) {
            Ok(m) if !m.is_file() || m.nlink() != 1 => {
                return Err(Error::new(
                    ErrorCode::Index,
                    "cache must contain ordinary singly-linked files",
                ));
            }
            Ok(_) => {
                filesystem::check_chain(&p)?;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}
fn open(path: &Path) -> Result<(Connection, bool)> {
    filesystem::ensure_directory(path.parent().unwrap())?;
    fs::set_permissions(path.parent().unwrap(), fs::Permissions::from_mode(0o700))?;
    secure_cache(path)?;
    let existed = path.exists();
    fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    let attempt = (|| -> rusqlite::Result<Connection> {
        let conn = connect(path)?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version == 0 {
            conn.execute_batch("BEGIN IMMEDIATE;")?;
            conn.execute_batch(include_str!("../migrations/001_index.sql"))?;
            conn.execute_batch("COMMIT;")?;
        } else if version != 1 {
            return Err(rusqlite::Error::InvalidQuery);
        }
        conn.prepare(
            "SELECT path, id, title, body, tags_json, content_hash, fingerprint, stale FROM files",
        )?;
        // Check actual health on every opening, including status/search. Otherwise
        // untouched FTS corruption could be mislabeled as a healthy warm index.
        {
            let health: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
            if health != "ok" {
                return Err(rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(11),
                    Some(health),
                ));
            }
            conn.execute(
                "INSERT INTO notes_fts(notes_fts) VALUES('integrity-check')",
                [],
            )?;
        }
        Ok(conn)
    })();
    match attempt {
        Ok(conn) => Ok((conn, !existed)),
        Err(e) if corrupt(&e) => {
            // Every Foglio connection holds the same lock; no live app readers or WAL writers.
            secure_cache(path)?;
            for suffix in ["-journal", "-wal", "-shm", ""] {
                let p = PathBuf::from(format!("{}{suffix}", path.display()));
                match fs::remove_file(p) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => return Err(e.into()),
                }
            }
            let (conn, _) = open(path)?;
            Ok((conn, true))
        }
        Err(e) => Err(db_error(e)),
    }
}
fn fingerprint(m: &fs::Metadata) -> String {
    format!(
        "{}:{}:{}:{}:{}:{}:{}:{}",
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
        m.mode()
    )
}
struct Scan<'a> {
    root: &'a Path,
    old: &'a HashMap<String, Cached>,
    found: HashMap<String, Cached>,
    unknown: Vec<String>,
    force: bool,
    status: IndexStatus,
}
impl Scan<'_> {
    fn diagnostic(&mut self, path: String, e: Error, unknown: bool) {
        if unknown {
            self.unknown.push(path.clone());
        }
        self.status.diagnostics.push(Diagnostic {
            path,
            code: e.code,
            message: e.message,
        });
    }
    fn walk(&mut self, dir: &Path) {
        let relative = dir
            .strip_prefix(self.root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let entries = match filesystem::check_chain(dir).and_then(|()| Ok(fs::read_dir(dir)?)) {
            Ok(entries) => entries,
            Err(e) => {
                self.diagnostic(relative, e, true);
                return;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    self.diagnostic(relative.clone(), e.into(), true);
                    continue;
                }
            };
            let p = entry.path();
            let rel = p
                .strip_prefix(self.root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            let result = (|| -> Result<()> {
                let m = fs::symlink_metadata(&p)?;
                if m.file_type().is_symlink() {
                    return Err(Error::new(ErrorCode::Unsupported, "symlink not followed"));
                }
                if p.to_str().is_none() {
                    return Err(Error::new(ErrorCode::Path, "non-UTF-8 path"));
                }
                if m.is_dir() {
                    self.walk(&p);
                    return Ok(());
                }
                if entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".foglio-stage-")
                    && p.extension().is_some_and(|e| e == "tmp")
                {
                    return Ok(());
                }
                if !m.is_file() || p.extension().is_none_or(|e| e != "md") {
                    return Err(Error::new(
                        ErrorCode::Skipped,
                        "not a regular lowercase .md note",
                    ));
                }
                filesystem::relative(&rel)?;
                self.status.discovered_notes += 1;
                let fp = fingerprint(&m);
                if !self.force
                    && let Some(old) = self.old.get(&rel)
                    && !old.stale
                    && old.fingerprint == fp
                {
                    // Check actual access even for cached bodies; metadata alone is not readability.
                    let f = fs::OpenOptions::new()
                        .read(true)
                        .custom_flags(
                            rustix::fs::OFlags::NOFOLLOW.bits() as i32
                                | rustix::fs::OFlags::NONBLOCK.bits() as i32,
                        )
                        .open(&p)?;
                    if fingerprint(&f.metadata()?) != fp {
                        return Err(Error::new(ErrorCode::Conflict, "file changed during scan"));
                    }
                    self.found.insert(rel.clone(), old.clone());
                    self.status.reused_notes += 1;
                    return Ok(());
                }
                let bytes = filesystem::read(&p)?;
                self.status.parsed_notes += 1;
                let doc = Document::parse(&bytes, p.file_stem().unwrap().to_str().unwrap())?;
                if fingerprint(&fs::symlink_metadata(&p)?) != fp {
                    return Err(Error::new(ErrorCode::Conflict, "file changed during scan"));
                }
                let id = doc
                    .id
                    .ok_or_else(|| Error::new(ErrorCode::MissingId, "run init to adopt"))?;
                self.found.insert(
                    rel.clone(),
                    Cached {
                        path: rel.clone(),
                        id: id.as_str().into(),
                        title: doc.title,
                        body: doc.body,
                        tags_json: serde_json::to_string(&doc.tags).unwrap(),
                        hash: doc.revision.to_string(),
                        fingerprint: fp,
                        stale: false,
                    },
                );
                Ok(())
            })();
            if let Err(e) = result {
                let unknown = !matches!(e.code, ErrorCode::Skipped | ErrorCode::MissingId);
                self.diagnostic(rel, e, unknown);
            }
        }
    }
}
fn covered(path: &str, parent: &str) -> bool {
    parent.is_empty()
        || path == parent
        || path
            .strip_prefix(parent)
            .is_some_and(|s| s.starts_with('/'))
}

impl Library {
    pub fn cache_path(&self) -> Result<PathBuf> {
        let path = self
            .cache
            .join(format!(
                "{}",
                crate::revision(self.root().as_os_str().as_encoded_bytes())
            ))
            .join("index.sqlite3");
        filesystem::check_chain(&path)?;
        if path.starts_with(self.root()) || self.root().starts_with(&self.cache) {
            return Err(Error::new(
                ErrorCode::Config,
                "cache and library must not overlap",
            ));
        }
        Ok(path)
    }
    pub fn status(&self) -> Result<IndexStatus> {
        let _lock = self.lock()?;
        self.reconcile_locked(false, false)
            .map(|(_, status)| status)
    }
    /// Hash and parse every note; does not adopt or rewrite Markdown.
    pub fn rescan(&self) -> Result<IndexStatus> {
        let _lock = self.lock()?;
        self.reconcile_locked(true, false).map(|(_, status)| status)
    }
    /// Reconstruct derived metadata, tags and FTS in one transaction.
    pub fn reindex(&self) -> Result<IndexStatus> {
        let _lock = self.lock()?;
        self.reconcile_locked(true, true).map(|(_, status)| status)
    }
    pub(crate) fn reconcile_locked(
        &self,
        force: bool,
        rebuild: bool,
    ) -> Result<(Connection, IndexStatus)> {
        let (mut conn, cache_rebuilt) = open(&self.cache_path()?)?;
        let old: HashMap<String, Cached> = {
            let mut stmt = conn
                .prepare(
                    "SELECT path,id,title,body,tags_json,content_hash,fingerprint,stale FROM files",
                )
                .map_err(db_error)?;
            let rows = stmt
                .query_map([], |r| {
                    Ok(Cached {
                        path: r.get(0)?,
                        id: r.get(1)?,
                        title: r.get(2)?,
                        body: r.get(3)?,
                        tags_json: r.get(4)?,
                        hash: r.get(5)?,
                        fingerprint: r.get(6)?,
                        stale: r.get(7)?,
                    })
                })
                .map_err(db_error)?;
            rows.map(|r| r.map(|c| (c.path.clone(), c)))
                .collect::<rusqlite::Result<_>>()
                .map_err(db_error)?
        };
        let mut scan = Scan {
            root: self.root(),
            old: &old,
            found: HashMap::new(),
            unknown: Vec::new(),
            force: force || cache_rebuilt,
            status: IndexStatus {
                cache_rebuilt,
                watcher_active: self
                    .watcher_backends
                    .load(std::sync::atomic::Ordering::Acquire)
                    > 0,
                ..IndexStatus::default()
            },
        };
        scan.walk(self.root());
        for (path, record) in &old {
            if !scan.found.contains_key(path) && scan.unknown.iter().any(|p| covered(path, p)) {
                let mut stale = record.clone();
                stale.stale = true;
                scan.found.insert(path.clone(), stale);
            }
        }
        let mut ids: HashMap<&str, usize> = HashMap::new();
        for c in scan.found.values() {
            *ids.entry(&c.id).or_default() += 1;
        }
        let identity_unknown = !scan.unknown.is_empty();
        let mut eligible = HashSet::new();
        for c in scan.found.values() {
            if c.stale {
                scan.status.stale_notes += 1;
            }
            if ids[c.id.as_str()] > 1 {
                scan.status.ambiguous_notes += 1;
                scan.status.diagnostics.push(Diagnostic {
                    path: c.path.clone(),
                    code: ErrorCode::Ambiguous,
                    message: "duplicate id: every member excluded from search".into(),
                });
            } else if !c.stale && !identity_unknown {
                eligible.insert(c.path.clone());
            }
        }
        // Unknown identity anywhere can conceal a duplicate of any otherwise-valid note.
        if identity_unknown {
            scan.status.diagnostics.push(Diagnostic { path:String::new(), code:ErrorCode::Incomplete, message:"identity map incomplete; search eligibility paused until reconciliation succeeds".into() });
        }
        let tx = conn.transaction().map_err(db_error)?;
        if rebuild {
            tx.execute_batch("DELETE FROM notes; DELETE FROM notes_fts; DELETE FROM files;")
                .map_err(db_error)?;
        }
        let previous: HashSet<String> = {
            let mut stmt = tx.prepare("SELECT path FROM notes").map_err(db_error)?;
            stmt.query_map([], |r| r.get(0))
                .map_err(db_error)?
                .collect::<rusqlite::Result<_>>()
                .map_err(db_error)?
        };
        for path in old.keys() {
            if !scan.found.contains_key(path) {
                tx.execute("DELETE FROM files WHERE path=?1", [path])
                    .map_err(db_error)?;
            }
        }
        // Delete affected old identities first, allowing paths and IDs to swap atomically.
        for path in &previous {
            if !eligible.contains(path) || old.get(path) != scan.found.get(path) {
                tx.execute("DELETE FROM notes WHERE path=?1", [path])
                    .map_err(db_error)?;
            }
        }
        for c in scan.found.values() {
            if rebuild || old.get(&c.path) != Some(c) {
                tx.execute(
                    "INSERT OR REPLACE INTO files VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                    params![
                        c.path,
                        c.id,
                        c.title,
                        c.body,
                        c.tags_json,
                        c.hash,
                        c.fingerprint,
                        c.stale
                    ],
                )
                .map_err(db_error)?;
            }
            if eligible.contains(&c.path)
                && (rebuild || !previous.contains(&c.path) || old.get(&c.path) != Some(c))
            {
                tx.execute(
                    "INSERT INTO notes VALUES(?1,?2,?3,?4)",
                    params![c.id, c.path, c.title, c.hash],
                )
                .map_err(db_error)?;
                let tags: Vec<String> = serde_json::from_str(&c.tags_json)
                    .map_err(|e| Error::new(ErrorCode::Index, e))?;
                for tag in &tags {
                    tx.execute("INSERT INTO tags VALUES(?1,?2)", params![c.id, tag])
                        .map_err(db_error)?;
                }
                tx.execute(
                    "INSERT INTO notes_fts(note_id,title,body,path,tags) VALUES(?1,?2,?3,?4,?5)",
                    params![c.id, c.title, c.body, c.path, tags.join(" ")],
                )
                .map_err(db_error)?;
            }
        }
        tx.commit().map_err(db_error)?;
        scan.status.indexed_notes = eligible.len();
        scan.status.incomplete = !scan.status.diagnostics.is_empty();
        scan.status.diagnostics.sort_by(|a, b| {
            a.path
                .cmp(&b.path)
                .then(a.code.as_str().cmp(b.code.as_str()))
        });
        Ok((conn, scan.status))
    }
    /// Capture body-free identities from the exact committed index generation.
    /// The connection is closed before releasing the cooperative lock.
    pub(crate) fn watch_reconcile(&self) -> Result<(IndexStatus, Vec<crate::events::WatchedNote>)> {
        let _lock = self.lock()?;
        let (conn, status) = self.reconcile_locked(true, false)?;
        let mut stmt = conn
            .prepare("SELECT id,path,content_hash FROM notes ORDER BY path")
            .map_err(db_error)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .map_err(db_error)?;
        let mut notes = Vec::new();
        for row in rows {
            let (id, path, hash) = row.map_err(db_error)?;
            notes.push(crate::events::WatchedNote {
                id: crate::NoteId::parse(&id)?,
                path,
                revision: crate::Revision(hash),
            });
        }
        Ok((status, notes))
    }
    pub(crate) fn index_after_commit(
        &self,
        commit: filesystem::Commit,
    ) -> Result<filesystem::Commit> {
        match self.reconcile_locked(false, false) {
            Ok((_, status)) if !status.incomplete => Ok(commit),
            Ok(_) => Err(Error::committed(
                "file committed; index degraded by scan diagnostics; run status/rescan",
                commit,
            )),
            Err(e) => Err(Error::committed(
                &format!("file committed; index degraded: {e}; run rescan/reindex"),
                commit,
            )),
        }
    }
}
