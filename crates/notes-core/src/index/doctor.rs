//! Read-only diagnostics. Never use `open`, `lock`, or reconciliation here.
//! The immutable SQLite source cannot create journals/SHM or checkpoint WAL.
//! Sidecars are refused; all SQL health checks run against an in-memory copy.
use super::*;

#[derive(Debug, Clone, Serialize)]
pub struct DoctorDiagnostic {
    pub path: String,
    pub code: ErrorCode,
    pub kind: String,
    pub message: String,
    pub action: String,
}
#[derive(Debug, Default, Serialize)]
pub struct DoctorReport {
    pub root: PathBuf,
    pub cache_path: Option<PathBuf>,
    pub discovered_notes: usize,
    pub indexed_notes: usize,
    pub stale_notes: usize,
    pub orphan_records: usize,
    pub unindexed_notes: usize,
    pub incomplete: bool,
    /// False when cache access/schema/sidecars make automatic repair unsafe.
    pub reindex_safe: bool,
    pub diagnostics: Vec<DoctorDiagnostic>,
}
impl DoctorReport {
    fn issue(
        &mut self,
        path: &str,
        code: ErrorCode,
        kind: &str,
        message: impl ToString,
        action: &str,
    ) {
        self.diagnostics.push(DoctorDiagnostic {
            path: path.into(),
            code,
            kind: kind.into(),
            message: message.to_string(),
            action: action.into(),
        });
    }
}
fn sidecars(path: &Path) -> Result<bool> {
    for suffix in ["-journal", "-wal", "-shm"] {
        match fs::symlink_metadata(format!("{}{suffix}", path.display())) {
            Ok(_) => return Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(false)
}
fn snapshot(path: &Path) -> Result<Connection> {
    secure_cache(path)?;
    if sidecars(path)? {
        return Err(Error::new(
            ErrorCode::Busy,
            "cache sidecars present; close other Foglio/SQLite processes and retry",
        ));
    }
    let before = fingerprint(&fs::metadata(path)?);
    // Encode every byte, including '?' and '#', so path characters cannot inject URI options.
    let uri = format!(
        "file:{}?mode=ro&immutable=1",
        path.as_os_str()
            .as_encoded_bytes()
            .iter()
            .map(|b| format!("%{b:02X}"))
            .collect::<String>()
    );
    let source = Connection::open_with_flags(
        uri,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .map_err(db_error)?;
    let mut memory = Connection::open_in_memory().map_err(db_error)?;
    {
        let backup = rusqlite::backup::Backup::new(&source, &mut memory).map_err(db_error)?;
        backup
            .run_to_completion(256, Duration::ZERO, None)
            .map_err(db_error)?;
    }
    if sidecars(path)? || fingerprint(&fs::metadata(path)?) != before {
        return Err(Error::new(
            ErrorCode::Busy,
            "cache changed during inspection; retry when writers are idle",
        ));
    }
    memory
        .execute_batch("PRAGMA trusted_schema=OFF;")
        .map_err(db_error)?;
    Ok(memory)
}
fn schema(conn: &Connection) -> rusqlite::Result<Vec<(String, String, String)>> {
    conn.prepare(
        "SELECT type,name,sql FROM sqlite_schema WHERE sql IS NOT NULL ORDER BY type,name",
    )?
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
    .collect()
}
fn check_cache(
    conn: &Connection,
    scan: &Scan<'_>,
    report: &mut DoctorReport,
) -> rusqlite::Result<()> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let expected = Connection::open_in_memory()?;
    expected.execute_batch(include_str!("../../migrations/002_index.sql"))?;
    if version != 2 || schema(conn)? != schema(&expected)? {
        report.reindex_safe = false;
        report.issue("", ErrorCode::Index, "cache_schema", format!("unsupported or damaged cache schema (version {version}, expected 2)"), "Do not overwrite unknown schemas. Use the matching Foglio version or manually remove only this disposable cache after closing all clients, then run reindex.");
        return Ok(());
    }
    let health: String = conn.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    if health != "ok" {
        return Err(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(11),
            Some(health),
        ));
    }
    // FTS5's integrity command is a write even when it changes nothing. Memory only!
    conn.execute(
        "INSERT INTO notes_fts(notes_fts) VALUES('integrity-check')",
        [],
    )?;
    let mut stmt = conn.prepare(
        "SELECT path,title,body,tags_json,content_hash,fingerprint,stale FROM files ORDER BY path",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Cached {
            path: r.get(0)?,
            title: r.get(1)?,
            body: r.get(2)?,
            tags_json: r.get(3)?,
            hash: r.get(4)?,
            fingerprint: r.get(5)?,
            stale: r.get(6)?,
        })
    })?;
    let mut cached_paths = HashSet::new();
    for row in rows {
        let old = row?;
        cached_paths.insert(old.path.clone());
        match scan.found.get(&old.path) {
            Some(current) if current != &old => {
                report.stale_notes += 1;
                report.issue(
                    &old.path,
                    ErrorCode::Index,
                    "stale_record",
                    "cached content/metadata differs from disk or is marked stale",
                    "Run notes reindex; source files are authoritative.",
                );
            }
            Some(_) => {}
            None if scan.unknown.iter().any(|p| covered(&old.path, p)) => {
                report.issue(
                    &old.path,
                    ErrorCode::Incomplete,
                    "unverified_record",
                    "source could not be checked; record is not a proven orphan",
                    "Resolve source diagnostics and rerun doctor before reindexing.",
                );
            }
            None => {
                report.orphan_records += 1;
                report.issue(
                    &old.path,
                    ErrorCode::Index,
                    "orphan_record",
                    "cached path no longer exists in the scanned library",
                    "Run notes reindex to remove derived orphan records.",
                );
            }
        }
    }
    for path in scan.found.keys() {
        if !cached_paths.contains(path) {
            report.unindexed_notes += 1;
            report.issue(
                path,
                ErrorCode::Index,
                "unindexed_note",
                "valid note absent from cache",
                "Run notes reindex.",
            );
        }
    }
    report.indexed_notes =
        conn.query_row("SELECT count(*) FROM notes", [], |r| r.get::<_, i64>(0))? as usize;
    // Compare both directions, catching missing/extra tags, eligibility and FTS rows.
    let inconsistent: bool = conn.query_row("SELECT
        EXISTS(SELECT path,title,content_hash FROM notes EXCEPT SELECT path,title,content_hash FROM files WHERE stale=0) OR
        EXISTS(SELECT path,title,content_hash FROM files WHERE stale=0 EXCEPT SELECT path,title,content_hash FROM notes) OR
        EXISTS(SELECT note_path,tag FROM tags EXCEPT SELECT f.path,j.value FROM files f,json_each(f.tags_json) j WHERE stale=0) OR
        EXISTS(SELECT f.path,j.value FROM files f,json_each(f.tags_json) j WHERE stale=0 EXCEPT SELECT note_path,tag FROM tags) OR
        EXISTS(SELECT note_path,title,body,path,tags FROM notes_fts EXCEPT SELECT path,title,body,path,(SELECT coalesce(group_concat(value,' '),'') FROM json_each(tags_json)) FROM files WHERE stale=0) OR
        EXISTS(SELECT path,title,body,path,(SELECT coalesce(group_concat(value,' '),'') FROM json_each(tags_json)) FROM files WHERE stale=0 EXCEPT SELECT note_path,title,body,path,tags FROM notes_fts) OR
        (SELECT count(*) FROM notes_fts)!=(SELECT count(*) FROM notes)", [], |r| r.get(0))?;
    if inconsistent {
        report.issue(
            "",
            ErrorCode::Index,
            "cache_inconsistent",
            "derived notes/tags/search tables disagree with cached files",
            "Run notes reindex.",
        );
    }
    Ok(())
}
impl Library {
    /// Inspect every note and the existing derived cache, without creating state,
    /// locks or cache files, changing permissions, or rewriting note bytes.
    /// This is a best-effort observation, not a cross-filesystem atomic snapshot;
    /// stop writers and rerun for a stable diagnosis. Reads may update OS atime.
    pub fn doctor(&self) -> Result<DoctorReport> {
        let old = HashMap::new();
        let mut scan = Scan {
            root: self.root(),
            old: &old,
            found: HashMap::new(),
            unknown: Vec::new(),
            force: true,
            status: IndexStatus::default(),
        };
        scan.walk(self.root());
        let mut report = DoctorReport {
            root: self.root().to_path_buf(),
            cache_path: self.cache_path().ok(),
            discovered_notes: scan.status.discovered_notes,
            reindex_safe: true,
            ..DoctorReport::default()
        };
        for d in &scan.status.diagnostics {
            let kind = match d.code {
                ErrorCode::Metadata => "source_metadata",
                ErrorCode::Io | ErrorCode::Permission => "source_access",
                _ => "source_invalid",
            };
            report.issue(&d.path, d.code, kind, &d.message, "Inspect this source path and its permissions/metadata manually; doctor never rewrites notes.");
        }
        match self.cache_path() {
            Err(e) => {
                report.reindex_safe = false;
                report.issue(
                    "",
                    e.code,
                    "cache_access",
                    e.message,
                    "Correct cache location/access without changing source notes.",
                );
            }
            Ok(path) => match fs::symlink_metadata(&path) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => report.issue(
                    "",
                    ErrorCode::Index,
                    "cache_missing",
                    "no existing index cache",
                    "Run notes reindex to create disposable derived state.",
                ),
                Err(e) => {
                    report.reindex_safe = false;
                    report.issue(
                        "",
                        ErrorCode::Io,
                        "cache_access",
                        e,
                        "Restore read access to the cache and rerun doctor.",
                    );
                }
                Ok(_) => match snapshot(&path) {
                    Ok(conn) => {
                        if let Err(e) = check_cache(&conn, &scan, &mut report) {
                            report.issue("", ErrorCode::Index, "cache_corrupt", e, "Run notes reindex; if it refuses structural damage, close clients and manually remove only the disposable cache.");
                        }
                    }
                    Err(e) => {
                        let kind = if e.code == ErrorCode::Busy {
                            "cache_busy"
                        } else if e.code == ErrorCode::Index
                            && (e.message.contains("not a database")
                                || e.message.contains("malformed"))
                        {
                            "cache_corrupt"
                        } else {
                            "cache_access"
                        };
                        report.reindex_safe = kind == "cache_corrupt";
                        report.issue("", e.code, kind, e.message, if kind == "cache_corrupt" { "Run notes reindex to rebuild the disposable cache." } else { "Close writers, resolve cache access/unsafe paths, and rerun doctor; no automatic repair attempted." });
                    }
                },
            },
        }
        report
            .diagnostics
            .sort_by(|a, b| a.path.cmp(&b.path).then(a.kind.cmp(&b.kind)));
        report.incomplete = !report.diagnostics.is_empty();
        Ok(report)
    }
}
