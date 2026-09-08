use crate::ErrorCode;
use crate::{Document, Error, NoteId, Result, filesystem as fs, revision};
use serde::Serialize;
use std::{
    collections::HashMap,
    fs as disk,
    path::{Path, PathBuf},
};
#[derive(Debug, Clone, Serialize)]
pub struct Entry {
    pub path: String,
    #[serde(flatten)]
    pub document: Document,
    pub ambiguous: bool,
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
    pub id: Option<NoteId>,
    pub path: String,
    pub title: String,
    pub tags: Vec<String>,
    pub ambiguous: bool,
}
impl Report {
    pub fn summaries(&self) -> Vec<NoteSummary> {
        self.notes
            .iter()
            .map(|entry| NoteSummary {
                id: entry.document.id.clone(),
                path: entry.path.clone(),
                title: entry.document.title.clone(),
                tags: entry.document.tags.clone(),
                ambiguous: entry.ambiguous,
            })
            .collect()
    }
}
#[derive(Debug)]
pub struct Library {
    root: PathBuf,
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
        let report = lib.scan(true)?;
        Ok((lib, report))
    }
    pub fn lock(&self) -> Result<fs::LibraryLock> {
        fs::check_chain(&self.root)?;
        fs::lock(&self.state.join("locks").join(format!(
            "{}.lock",
            revision(self.root.as_os_str().as_encoded_bytes())
        )))
    }
    pub fn scan(&self, adopt: bool) -> Result<Report> {
        let _lock = if adopt { Some(self.lock()?) } else { None };
        let mut report = Report::default();
        self.walk(&self.root, &mut report, adopt)?;
        report.notes.sort_by(|a, b| a.path.cmp(&b.path));
        let mut ids = HashMap::<String, usize>::new();
        for e in &report.notes {
            if let Some(id) = &e.document.id {
                *ids.entry(id.as_str().into()).or_default() += 1;
            }
        }
        for e in &mut report.notes {
            if e.document
                .id
                .as_ref()
                .is_some_and(|id| ids[id.as_str()] > 1)
            {
                e.ambiguous = true;
                report.diagnostics.push(Diagnostic {
                    path: e.path.clone(),
                    code: ErrorCode::Ambiguous,
                    message: "duplicate id: every member is ambiguous".into(),
                });
            }
        }
        if adopt {
            match self.reconcile_locked(false, false) {
                Ok((_, status)) => {
                    if status.incomplete && report.diagnostics.is_empty() {
                        report.diagnostics.extend(status.diagnostics);
                    }
                }
                Err(e) => report.diagnostics.push(Diagnostic {
                    path: String::new(),
                    code: ErrorCode::Index,
                    message: e.to_string(),
                }),
            }
        }
        report.incomplete = !report.diagnostics.is_empty();
        Ok(report)
    }
    fn walk(&self, dir: &Path, r: &mut Report, adopt: bool) -> Result<()> {
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
                    return self.walk(&p, r, adopt);
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
                let mut doc =
                    Document::parse(&fs::read(&p)?, p.file_stem().unwrap().to_str().unwrap())?;
                if doc.id.is_none() {
                    if adopt {
                        let source = doc.with_id(&NoteId::generate());
                        let candidate = Document::parse(source.as_bytes(), &doc.title)?;
                        let outcome = fs::save(&p, source.as_bytes(), Some(&doc.revision))?;
                        doc = candidate;
                        if !outcome.durability_confirmed {
                            r.diagnostics.push(Diagnostic {
                                path: relative.clone(),
                                code: ErrorCode::Committed,
                                message: "adopted; durability uncertain".into(),
                            });
                        }
                    } else {
                        return Err(Error::new(ErrorCode::MissingId, "run init to adopt"));
                    }
                }
                r.notes.push(Entry {
                    path: relative.clone(),
                    document: doc,
                    ambiguous: false,
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
    pub fn get(&self, selector: &str) -> Result<Entry> {
        let r = self.scan(false)?;
        if let Some(p) = selector.strip_prefix("path:") {
            return r
                .notes
                .iter()
                .find(|e| e.path == p)
                .cloned()
                .map(Ok)
                .unwrap_or_else(|| self.read_entry(p));
        }
        let explicit = selector.strip_prefix("id:");
        let id = if let Some(s) = explicit {
            Some(NoteId::parse(s)?)
        } else {
            NoteId::parse(selector).ok()
        };
        let by_id: Vec<_> = r
            .notes
            .iter()
            .filter(|e| {
                id.as_ref()
                    .is_some_and(|id| e.document.id.as_ref() == Some(id))
            })
            .collect();
        let by_path = if explicit.is_none() {
            r.notes.iter().find(|e| e.path == selector)
        } else {
            None
        };
        if by_id.len() > 1 || (!by_id.is_empty() && by_path.is_some()) {
            return Err(Error::new(
                ErrorCode::Ambiguous,
                "use an explicit path for inspection",
            ));
        }
        if let Some(e) = by_id.first().copied().or(by_path) {
            return Ok(e.clone());
        }
        if explicit.is_none() && selector.ends_with(".md") {
            return self.read_entry(selector);
        }
        Err(Error::new(ErrorCode::NotFound, "note not found"))
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
            ambiguous: false,
        })
    }
    fn mutation_target(&self, selector: &str, expected: &crate::Revision) -> Result<Entry> {
        let e = self.get(selector)?;
        let id = e
            .document
            .id
            .as_ref()
            .ok_or_else(|| Error::new(ErrorCode::MissingId, "run init first"))?;
        let r = self.scan(false)?;
        if r.notes
            .iter()
            .filter(|n| n.document.id.as_ref() == Some(id))
            .count()
            != 1
        {
            return Err(Error::new(ErrorCode::Ambiguous, "duplicate identity"));
        }
        if r.diagnostics
            .iter()
            .any(|d| !matches!(d.code.as_str(), "skipped" | "missing_id" | "ambiguous"))
        {
            return Err(Error::new(
                ErrorCode::Incomplete,
                "cannot establish complete identity map",
            ));
        }
        fs::precondition(&fs::safe_path(&self.root, &e.path)?, expected)?;
        Ok(e)
    }
    pub fn create(&self, path: &str, body: &str, tags: &[String]) -> Result<Entry> {
        let _lock = self.lock()?;
        let path_abs = fs::safe_path(&self.root, path)?;
        let id = NoteId::generate();
        let source = format!(
            "---\nid: {}\ntags: {}\n---\n{}",
            id.as_str(),
            serde_json::to_string(tags).unwrap(),
            body
        );
        let document = Document::parse(source.as_bytes(), path)?;
        let commit = fs::save(&path_abs, source.as_bytes(), None)?;
        let commit = self.index_after_commit(commit)?;
        if !commit.durability_confirmed {
            return Err(Error::committed("created; durability uncertain", commit));
        }
        Ok(Entry {
            path: path.into(),
            document,
            ambiguous: false,
        })
    }
    pub fn update(
        &self,
        selector: &str,
        expected: &crate::Revision,
        body: &str,
    ) -> Result<fs::Commit> {
        let _lock = self.lock()?;
        let e = self.mutation_target(selector, expected)?;
        let source = e.document.with_body(body);
        Document::parse(source.as_bytes(), &e.document.title)?;
        let commit = fs::save(
            &fs::safe_path(&self.root, &e.path)?,
            source.as_bytes(),
            Some(expected),
        )?;
        self.index_after_commit(commit)
    }
    pub fn tag(
        &self,
        selector: &str,
        expected: &crate::Revision,
        tag: &str,
        add: bool,
    ) -> Result<fs::Commit> {
        let _lock = self.lock()?;
        let e = self.mutation_target(selector, expected)?;
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
        selector: &str,
        expected: &crate::Revision,
        to: &str,
    ) -> Result<fs::Commit> {
        let _lock = self.lock()?;
        let e = self.mutation_target(selector, expected)?;
        let commit = fs::move_file(
            &fs::safe_path(&self.root, &e.path)?,
            &fs::safe_path(&self.root, to)?,
            expected,
        )?;
        self.index_after_commit(commit)
    }
    pub fn delete(&self, selector: &str, expected: &crate::Revision) -> Result<fs::Commit> {
        let _lock = self.lock()?;
        let e = self.mutation_target(selector, expected)?;
        let commit = fs::delete_file(&fs::safe_path(&self.root, &e.path)?, expected)?;
        self.index_after_commit(commit)
    }
}
