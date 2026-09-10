use crate::ErrorCode;
use crate::{Document, Error, Result, filesystem as fs, revision};
use serde::Serialize;
use std::{
    fs as disk,
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub path: String,
    #[serde(flatten)]
    pub document: Document,
}
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub path: String,
    pub code: ErrorCode,
    pub message: String,
}
#[derive(Debug, Default, Serialize)]
pub struct Report {
    pub notes: Vec<Entry>,
    pub diagnostics: Vec<Diagnostic>,
    pub incomplete: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct NoteSummary {
    pub path: String,
    pub title: String,
    pub tags: Vec<String>,
}
impl Report {
    pub fn summaries(&self) -> Vec<NoteSummary> {
        self.notes
            .iter()
            .map(|entry| NoteSummary {
                path: entry.path.clone(),
                title: entry.document.title.clone(),
                tags: entry.document.tags.clone(),
            })
            .collect()
    }
}
#[derive(Debug)]
pub struct Library {
    root: PathBuf,
    local: std::sync::Arc<crate::coordination::Gate>,
    state: PathBuf,
    pub(crate) cache: PathBuf,
    pub(crate) watcher_backends: std::sync::atomic::AtomicUsize,
}
fn absolute(p: &Path) -> Result<PathBuf> {
    Ok(if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()?.join(p)
    })
}
pub fn config_dir() -> Result<PathBuf> {
    let base = if let Some(p) = std::env::var_os("XDG_CONFIG_HOME") {
        let p = PathBuf::from(p);
        if !p.is_absolute() {
            return Err(Error::new(
                ErrorCode::Config,
                "XDG_CONFIG_HOME must be absolute",
            ));
        }
        p
    } else {
        PathBuf::from(
            std::env::var_os("HOME")
                .ok_or_else(|| Error::new(ErrorCode::Config, "HOME unavailable"))?,
        )
        .join(".config")
    };
    if !base.is_absolute() {
        return Err(Error::new(ErrorCode::Config, "HOME must be absolute"));
    }
    let p = base.join("foglio");
    fs::check_chain(&p)?;
    Ok(p)
}
pub fn cache_dir() -> Result<PathBuf> {
    let base = if let Some(p) = std::env::var_os("XDG_CACHE_HOME") {
        PathBuf::from(p)
    } else {
        PathBuf::from(
            std::env::var_os("HOME")
                .ok_or_else(|| Error::new(ErrorCode::Config, "HOME unavailable"))?,
        )
        .join(".cache")
    };
    if !base.is_absolute() {
        return Err(Error::new(ErrorCode::Config, "cache base must be absolute"));
    }
    let path = base.join("foglio");
    fs::check_chain(&path)?;
    Ok(path)
}
impl Library {
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn open(root: &Path, state: &Path, create: bool) -> Result<Self> {
        let root = absolute(root)?;
        let state = absolute(state)?;
        if root.to_str().is_none() || state.to_str().is_none() {
            return Err(Error::new(
                ErrorCode::Path,
                "library and state paths must be UTF-8",
            ));
        }
        fs::check_chain(&root)?;
        fs::check_chain(&state)?;
        if root.starts_with(&state) || state.starts_with(&root) {
            return Err(Error::new(
                ErrorCode::Config,
                "library and application state must not overlap",
            ));
        }
        if create {
            fs::ensure_directory(&root)?;
        }
        let root = disk::canonicalize(root)?;
        if !root.is_dir() {
            return Err(Error::new(ErrorCode::Path, "library is not directory"));
        }
        let cache = state.join("cache");
        Ok(Self {
            root,
            local: Default::default(),
            state,
            cache,
            watcher_backends: std::sync::atomic::AtomicUsize::new(0),
        })
    }
    pub fn resolve(override_root: Option<&Path>) -> Result<Self> {
        let state = config_dir()?;
        let root = if let Some(p) = override_root {
            p.to_path_buf()
        } else {
            let b = fs::read(&state.join("config.json"))?;
            serde_json::from_slice::<PathBuf>(&b)
                .map_err(|_| Error::new(ErrorCode::Config, "invalid selected root"))?
        };
        let mut lib = Self::open(&root, &state, false)?;
        lib.cache = cache_dir()?;
        lib.cache_path()?;
        Ok(lib)
    }
    /// Persist this validated existing root without changing notes.
    /// Uses the same locked, revision-checked durable config format as CLI init.
    pub fn select_existing(&self) -> Result<()> {
        fs::check_chain(&self.root)?;
        if !self.root.is_dir() {
            return Err(Error::new(ErrorCode::Path, "library is not directory"));
        }
        let _selection = fs::lock(&self.state.join("selection.lock"))?;
        let config = self.state.join("config.json");
        let bytes = serde_json::to_vec(&self.root).map_err(|e| Error::new(ErrorCode::Config, e))?;
        let expected = if config.exists() {
            Some(revision(&fs::read(&config)?))
        } else {
            None
        };
        let outcome = fs::save(&config, &bytes, expected.as_ref())?;
        if !outcome.durability_confirmed {
            return Err(Error::committed(
                "root selected but durability uncertain",
                outcome,
            ));
        }
        Ok(())
    }
    pub fn init(root: &Path) -> Result<(Self, Report)> {
        let state = config_dir()?;
        let mut lib = Self::open(root, &state, true)?;
        lib.cache = cache_dir()?;
        lib.cache_path()?;
        let _selection = fs::lock(&state.join("selection.lock"))?;
        let config = state.join("config.json");
        let bytes = serde_json::to_vec(&lib.root).map_err(|e| Error::new(ErrorCode::Config, e))?;
        let expected = if config.exists() {
            Some(revision(&fs::read(&config)?))
        } else {
            None
        };
        let outcome = fs::save(&config, &bytes, expected.as_ref())?;
        if !outcome.durability_confirmed {
            return Err(Error::committed(
                "root selected but durability uncertain",
                outcome,
            ));
        }
        let report = lib.scan()?;
        Ok((lib, report))
    }
    pub fn lock(&self) -> Result<fs::LibraryLock> {
        let local = self.local.acquire()?;
        fs::check_chain(&self.root)?;
        let lock = fs::lock(&self.state.join("locks").join(format!(
            "{}.lock",
            revision(self.root.as_os_str().as_encoded_bytes())
        )))?;
        Ok(lock.with_local(local))
    }
    pub fn scan(&self) -> Result<Report> {
        let mut report = Report::default();
        self.walk(&self.root, &mut report)?;
        report.notes.sort_by(|a, b| a.path.cmp(&b.path));
        report.incomplete = !report.diagnostics.is_empty();
        Ok(report)
    }
    fn walk(&self, dir: &Path, r: &mut Report) -> Result<()> {
        fs::check_chain(dir)?;
        for ent in disk::read_dir(dir)? {
            let ent = match ent {
                Ok(e) => e,
                Err(e) => {
                    r.diagnostics.push(Diagnostic {
                        path: dir.to_string_lossy().into(),
                        code: ErrorCode::Io,
                        message: e.to_string(),
                    });
                    continue;
                }
            };
            let p = ent.path();
            let relative = p
                .strip_prefix(&self.root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            let result = (|| -> Result<()> {
                let meta = disk::symlink_metadata(&p)?;
                if meta.file_type().is_symlink() {
                    return Err(Error::new(ErrorCode::Unsupported, "symlink not followed"));
                }
                if p.to_str().is_none() {
                    return Err(Error::new(ErrorCode::Path, "non-UTF-8 path"));
                }
                if meta.is_dir() {
                    return self.walk(&p, r);
                }
                if ent
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".foglio-stage-")
                    && p.extension().is_some_and(|x| x == "tmp")
                {
                    return Ok(());
                }
                if !meta.is_file() || p.extension().is_none_or(|x| x != "md") {
                    return Err(Error::new(
                        ErrorCode::Skipped,
                        "not a regular lowercase .md note",
                    ));
                }
                fs::relative(&relative)?;
                let doc =
                    Document::parse(&fs::read(&p)?, p.file_stem().unwrap().to_str().unwrap())?;
                r.notes.push(Entry {
                    path: relative.clone(),
                    document: doc,
                });
                Ok(())
            })();
            if let Err(e) = result {
                r.diagnostics.push(Diagnostic {
                    path: relative,
                    code: e.code,
                    message: e.message,
                });
            }
        }
        Ok(())
    }
    /// Read a literal safe library-relative Markdown path.
    pub fn get(&self, path: &str) -> Result<Entry> {
        self.read_entry(path)
    }
    fn read_entry(&self, p: &str) -> Result<Entry> {
        let path = fs::safe_path(&self.root, p)?;
        if !path.exists() {
            return Err(Error::new(ErrorCode::NotFound, "note not found"));
        }
        let document = Document::parse(
            &fs::read(&path)?,
            path.file_stem().unwrap().to_str().unwrap(),
        )?;
        Ok(Entry {
            path: p.into(),
            document,
        })
    }
    fn mutation_target(&self, path: &str, expected: &crate::Revision) -> Result<Entry> {
        let e = self.get(path)?;
        if &e.document.revision != expected {
            return Err(Error::new(ErrorCode::Conflict, "note revision changed"));
        }
        fs::precondition(&fs::safe_path(&self.root, &e.path)?, expected)?;
        Ok(e)
    }
    pub fn create(&self, path: &str, body: &str, tags: &[String]) -> Result<Entry> {
        let _lock = self.lock()?;
        let path_abs = fs::safe_path(&self.root, path)?;
        let source = if tags.is_empty() {
            body.to_string()
        } else {
            format!(
                "---\ntags: {}\n---\n{}",
                serde_json::to_string(tags).unwrap(),
                body
            )
        };
        let document = Document::parse(
            source.as_bytes(),
            path_abs.file_stem().unwrap().to_str().unwrap(),
        )?;
        let commit = fs::save(&path_abs, source.as_bytes(), None)?;
        let commit = self.index_after_commit(commit)?;
        if !commit.durability_confirmed {
            return Err(Error::committed("created; durability uncertain", commit));
        }
        Ok(Entry {
            path: path.into(),
            document,
        })
    }
    pub fn update(&self, path: &str, expected: &crate::Revision, body: &str) -> Result<fs::Commit> {
        let _lock = self.lock()?;
        let e = self.mutation_target(path, expected)?;
        let source = e.document.with_body(body);
        Document::parse(source.as_bytes(), &e.document.title)?;
        let commit = fs::save(
            &fs::safe_path(&self.root, &e.path)?,
            source.as_bytes(),
            Some(expected),
        )?;
        self.index_after_commit(commit)
    }
    /// Save a draft beside its source without overwriting either observed bytes
    /// or a destination. `None` means the source was observed absent, not unchecked.
    pub fn save_copy(
        &self,
        source_path: &str,
        expected_source: Option<&crate::Revision>,
        destination: &str,
        base_source: &str,
        body: &str,
    ) -> Result<fs::Commit> {
        self.save_copy_observed(
            source_path,
            expected_source,
            destination,
            base_source,
            body,
            || Ok(()),
        )
    }
    fn save_copy_observed(
        &self,
        source_path: &str,
        expected_source: Option<&crate::Revision>,
        destination: &str,
        base_source: &str,
        body: &str,
        mut before_replace: impl FnMut() -> Result<()>,
    ) -> Result<fs::Commit> {
        let _lock = self.lock()?;
        let source_path_abs = fs::safe_path(&self.root, source_path)?;
        let destination_abs = fs::safe_path(&self.root, destination)?;
        if source_path.to_lowercase() == destination.to_lowercase() {
            return Err(Error::new(
                ErrorCode::Path,
                "copy destination must differ from source",
            ));
        }
        // Read raw bytes: source metadata may now be invalid, and copying never
        // requires source write permission. Do not confuse I/O failures with absence.
        let guard = || -> Result<()> {
            fs::check_chain(&source_path_abs)?;
            match disk::symlink_metadata(&source_path_abs) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    if expected_source.is_none() {
                        return Ok(());
                    }
                }
                Err(e) => return Err(e.into()),
                Ok(_) => {
                    if let Some(expected) = expected_source
                        && revision(&fs::read(&source_path_abs)?) == *expected
                    {
                        return Ok(());
                    }
                }
            }
            Err(Error::new(
                ErrorCode::Conflict,
                "copy source observation changed",
            ))
        };
        guard()?;
        let fallback = destination_abs.file_stem().unwrap().to_str().unwrap();
        let source = Document::parse(base_source.as_bytes(), fallback)?.with_body(body);
        Document::parse(source.as_bytes(), fallback)?;
        let commit = fs::save_observed(&destination_abs, source.as_bytes(), None, |stage| {
            if stage == fs::Stage::BeforeReplace {
                before_replace()?;
                guard()?;
            }
            Ok(())
        })?;
        self.index_after_commit(commit)
    }
    pub fn tag(
        &self,
        path: &str,
        expected: &crate::Revision,
        tag: &str,
        add: bool,
    ) -> Result<fs::Commit> {
        let _lock = self.lock()?;
        let e = self.mutation_target(path, expected)?;
        let mut tags = e.document.tags.clone();
        if add && !tags.iter().any(|t| t == tag) {
            tags.push(tag.into());
        }
        if !add {
            tags.retain(|t| t != tag);
        }
        let source = if tags == e.document.tags {
            e.document.source.clone()
        } else {
            e.document.with_tags(&tags)
        };
        Document::parse(source.as_bytes(), &e.document.title)?;
        let commit = fs::save(
            &fs::safe_path(&self.root, &e.path)?,
            source.as_bytes(),
            Some(expected),
        )?;
        self.index_after_commit(commit)
    }
    pub fn move_note(
        &self,
        path: &str,
        expected: &crate::Revision,
        to: &str,
    ) -> Result<fs::Commit> {
        let _lock = self.lock()?;
        let e = self.mutation_target(path, expected)?;
        let commit = fs::move_file(
            &fs::safe_path(&self.root, &e.path)?,
            &fs::safe_path(&self.root, to)?,
            expected,
        )?;
        self.index_after_commit(commit)
    }
    pub fn delete(&self, path: &str, expected: &crate::Revision) -> Result<fs::Commit> {
        let _lock = self.lock()?;
        let e = self.mutation_target(path, expected)?;
        let commit = fs::delete_file(&fs::safe_path(&self.root, &e.path)?, expected)?;
        self.index_after_commit(commit)
    }
}

#[cfg(test)]
mod copy_tests {
    use super::*;

    #[test]
    fn late_source_changes_and_destination_races_do_not_commit() {
        for scenario in ["changed", "removed", "reappeared", "collision"] {
            let temp = tempfile::tempdir().unwrap();
            let lib = Library::open(&temp.path().join("notes"), &temp.path().join("state"), true)
                .unwrap();
            let from = lib.root().join("a.md");
            let to = lib.root().join("copy.md");
            let expected = if scenario == "reappeared" {
                None
            } else {
                disk::write(&from, "old").unwrap();
                Some(revision(b"old"))
            };
            let result = lib.save_copy_observed(
                "a.md",
                expected.as_ref(),
                "copy.md",
                "old",
                "draft",
                || {
                    // The same library lock spans both guards and filesystem staging.
                    assert_eq!(lib.lock().unwrap_err().code, ErrorCode::Busy);
                    match scenario {
                        "removed" => disk::remove_file(&from)?,
                        "collision" => disk::write(&to, "other writer")?,
                        _ => disk::write(&from, "external")?,
                    }
                    Ok(())
                },
            );
            assert_eq!(
                result.unwrap_err().code,
                if scenario == "collision" {
                    ErrorCode::Exists
                } else {
                    ErrorCode::Conflict
                }
            );
            if scenario == "collision" {
                assert_eq!(disk::read_to_string(to).unwrap(), "other writer");
            } else {
                assert!(!to.exists());
            }
            assert!(disk::read_dir(lib.root()).unwrap().all(|e| {
                !e.unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".foglio-stage-")
            }));
        }
    }
}
