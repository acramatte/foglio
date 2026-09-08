//! Read-only desktop boundary. Slow core work and watcher joins run on blocking workers.
use notes_core::{
    Library, NoteId,
    library::{Diagnostic, NoteSummary},
    search::{SearchHit, SearchQuery},
    watcher::{Subscription, WatchOptions, Watcher},
};
use serde::Serialize;
use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

pub type Result<T> = std::result::Result<T, String>;
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Clone, Default, Debug, Serialize)]
pub struct DesktopState {
    pub session: u64,
    pub root: Option<String>,
    pub generation: u64,
    pub watcher_active: bool,
    pub error: Option<String>,
}
#[derive(Serialize)]
pub struct Browse {
    pub session: u64,
    pub generation: u64,
    pub root: String,
    pub notes: Vec<NoteSummary>,
    pub folders: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
    pub incomplete: bool,
}
#[derive(Serialize)]
pub struct Search {
    pub session: u64,
    pub hits: Vec<SearchHit>,
    pub incomplete: bool,
}
#[derive(Serialize)]
pub struct Note {
    pub session: u64,
    pub id: String,
    pub path: String,
    pub title: String,
    pub tags: Vec<String>,
    pub body: String,
    pub revision: notes_core::Revision,
}
#[derive(Serialize)]
pub struct Resolved {
    pub session: u64,
    pub id: String,
}
struct Selection {
    library: Arc<Library>,
    watcher: Watcher,
    subscription: Subscription,
}
struct Inner {
    // Never hold this mutex on the UI/runtime thread. State has its own tiny lock.
    selection: Mutex<Option<Selection>>,
    state: Mutex<DesktopState>,
    stopping: AtomicBool,
}
pub struct Backend {
    inner: Arc<Inner>,
    monitor: Option<thread::JoinHandle<()>>,
}
impl Backend {
    pub fn new() -> Result<Self> {
        let inner = Arc::new(Inner {
            selection: Mutex::new(None),
            state: Mutex::new(DesktopState::default()),
            stopping: AtomicBool::new(false),
        });
        let weak = Arc::downgrade(&inner);
        let monitor = thread::Builder::new()
            .name("foglio-desktop-events".into())
            .spawn(move || {
                while let Some(inner) = weak.upgrade() {
                    if inner.stopping.load(Ordering::Acquire) {
                        break;
                    }
                    if let Ok(selection) = inner.selection.try_lock()
                        && let Some(selected) = selection.as_ref()
                    {
                        refresh(&inner, selected);
                    }
                    drop(inner);
                    thread::park_timeout(Duration::from_millis(50));
                }
            })
            .map_err(err)?;
        Ok(Self {
            inner,
            monitor: Some(monitor),
        })
    }
    pub fn state(&self) -> DesktopState {
        self.inner.state.lock().unwrap().clone()
    }
    pub fn bootstrap(&self) {
        let mut selected = self.inner.selection.lock().unwrap();
        if self.state().session != 0 || self.inner.stopping.load(Ordering::Acquire) {
            return;
        }
        // Missing selection is normal onboarding; malformed/unreadable config is not.
        let result = (|| {
            let config = notes_core::library::config_dir()
                .map_err(err)?
                .join("config.json");
            match std::fs::symlink_metadata(&config) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(e) => return Err(err(e)),
                Ok(_) => {}
            }
            let library = Library::resolve(None).map_err(err)?;
            self.install(&mut selected, library, false).map(|_| ())
        })();
        if let Err(e) = result {
            self.inner.state.lock().unwrap().error = Some(e);
        }
    }
    pub fn select(&self, path: &Path) -> Result<DesktopState> {
        let mut selected = self.inner.selection.lock().unwrap();
        if self.inner.stopping.load(Ordering::Acquire) {
            return Err("desktop is closing".into());
        }
        let library = Library::resolve(Some(path)).map_err(err)?;
        self.install(&mut selected, library, true)
    }
    // Explicit state directory makes real-filesystem tests isolated, not a mock core.
    pub fn select_with_state(&self, path: &Path, state: &Path) -> Result<DesktopState> {
        let mut selected = self.inner.selection.lock().unwrap();
        if self.inner.stopping.load(Ordering::Acquire) {
            return Err("desktop is closing".into());
        }
        self.install(
            &mut selected,
            Library::open(path, state, false).map_err(err)?,
            true,
        )
    }
    fn install(
        &self,
        selected: &mut Option<Selection>,
        library: Library,
        persist: bool,
    ) -> Result<DesktopState> {
        let library = Arc::new(library);
        let watcher = Watcher::start(library.clone(), WatchOptions::default()).map_err(err)?;
        let subscription = watcher.subscribe(64).map_err(err)?;
        if persist {
            library.select_existing().map_err(err)?;
        }
        // Drop old native backend on this blocking worker, never on desktop_state.
        selected.take();
        *selected = Some(Selection {
            library,
            watcher,
            subscription,
        });
        {
            let mut state = self.inner.state.lock().unwrap();
            state.session += 1;
            state.root = Some(
                selected
                    .as_ref()
                    .unwrap()
                    .library
                    .root()
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        refresh(&self.inner, selected.as_ref().unwrap());
        Ok(self.state())
    }
    fn with_selection<T>(
        &self,
        session: u64,
        f: impl FnOnce(&Selection) -> Result<T>,
    ) -> Result<T> {
        let selected = self.inner.selection.lock().unwrap();
        if self.inner.stopping.load(Ordering::Acquire) {
            return Err("desktop is closing".into());
        }
        if self.state().session != session {
            return Err("stale library session".into());
        }
        let selected = selected.as_ref().ok_or("select a library first")?;
        refresh(&self.inner, selected);
        f(selected)
    }
    pub fn browse(&self, session: u64) -> Result<Browse> {
        self.with_selection(session, |s| {
            let generation = self.state().generation;
            let report = s.library.scan(false).map_err(err)?;
            let mut folders = Vec::new();
            let mut diagnostics = report.diagnostics.clone();
            collect_folders(
                s.library.root(),
                s.library.root(),
                &mut folders,
                &mut diagnostics,
            );
            folders.sort();
            folders.dedup();
            Ok(Browse {
                session,
                generation,
                root: s.library.root().to_string_lossy().into_owned(),
                notes: report.summaries(),
                folders,
                incomplete: report.incomplete
                    || !diagnostics.is_empty()
                    || self.state().error.is_some(),
                diagnostics,
            })
        })
    }
    pub fn search(
        &self,
        session: u64,
        query: String,
        tag: Option<String>,
        folder: Option<String>,
    ) -> Result<Search> {
        self.with_selection(session, |s| {
            let mut query = SearchQuery::literal(&query);
            query.tag = tag;
            query.folder = folder;
            let report = s.library.search(&query).map_err(err)?;
            Ok(Search {
                session,
                hits: report.hits,
                incomplete: report.status.incomplete || self.state().error.is_some(),
            })
        })
    }
    pub fn open(&self, session: u64, id: &str) -> Result<Note> {
        NoteId::parse(id).map_err(err)?;
        self.with_selection(session, |s| {
            let entry = s.library.get(&format!("id:{id}")).map_err(err)?;
            let d = entry.document;
            Ok(Note {
                session,
                id: id.into(),
                path: entry.path,
                title: d.title.clone(),
                tags: d.tags.clone(),
                body: d.body,
                revision: d.revision,
            })
        })
    }
    pub fn resolve_link(&self, session: u64, from: &str, target: &str) -> Result<Resolved> {
        let path = relative_link(from, target)?;
        self.with_selection(session, |s| {
            // Validate ancestors against symlinks, then use core's duplicate policy.
            notes_core::filesystem::safe_path(s.library.root(), &path).map_err(err)?;
            let entry = s.library.get(&format!("path:{path}")).map_err(err)?;
            if entry.ambiguous {
                return Err("ambiguous note link".into());
            }
            let id = entry.document.id.ok_or("linked note has no ID")?;
            let unique = s.library.get(&format!("id:{}", id.as_str())).map_err(err)?;
            if unique.path != path {
                return Err("note link changed during resolution".into());
            }
            Ok(Resolved {
                session,
                id: id.as_str().into(),
            })
        })
    }
    pub fn shutdown(&self) {
        self.inner.stopping.store(true, Ordering::Release);
        self.inner.selection.lock().unwrap().take();
        self.inner.state.lock().unwrap().watcher_active = false;
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        self.shutdown();
        if let Some(worker) = self.monitor.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}
fn refresh(inner: &Inner, selected: &Selection) {
    // Always consume sticky invalidations BEFORE snapshot; never rely on deltas alone.
    for _ in 0..64 {
        if selected.subscription.try_recv().is_none() {
            break;
        }
    }
    let snapshot = selected.watcher.snapshot();
    let mut state = inner.state.lock().unwrap();
    state.generation = snapshot.generation;
    state.watcher_active = snapshot.status.watcher_active && !selected.subscription.is_closed();
    state.error = snapshot.error;
}
fn collect_folders(
    root: &Path,
    dir: &Path,
    folders: &mut Vec<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let result = (|| -> std::io::Result<()> {
        notes_core::filesystem::check_chain(dir).map_err(std::io::Error::other)?;
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let path = entry.path();
                if let Some(relative) = path.strip_prefix(root).ok().and_then(Path::to_str) {
                    folders.push(relative.into());
                    collect_folders(root, &path, folders, diagnostics);
                }
            }
        }
        Ok(())
    })();
    if let Err(e) = result {
        diagnostics.push(Diagnostic {
            path: dir
                .strip_prefix(root)
                .unwrap_or(dir)
                .to_string_lossy()
                .into_owned(),
            code: notes_core::ErrorCode::Io,
            message: e.to_string(),
        });
    }
}
/// Decode once, reject residual escapes and URL syntax, normalize only contained parents.
pub fn relative_link(from: &str, target: &str) -> Result<String> {
    notes_core::filesystem::relative(from).map_err(err)?;
    if target.len() > 4096 || target.is_empty() {
        return Err("invalid note link".into());
    }
    let decoded = percent_encoding::percent_decode_str(target)
        .decode_utf8()
        .map_err(err)?;
    if decoded.contains(['%', '\\', ':', '?'])
        || decoded.chars().any(char::is_control)
        || decoded.starts_with('/')
    {
        return Err("unsafe note link".into());
    }
    let file = decoded.split('#').next().unwrap();
    if file.is_empty() {
        return Ok(from.into());
    }
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for part in file.split('/') {
        match part {
            "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err("link escapes library".into());
                }
            }
            "" => return Err("empty link component".into()),
            _ => parts.push(part),
        }
    }
    let path = parts.join("/");
    notes_core::filesystem::relative(&path).map_err(err)?;
    Ok(path)
}
pub fn external_url(input: &str) -> Result<url::Url> {
    if input.len() > 4096
        || input.chars().any(char::is_control)
        || input.trim() != input
        || input.contains('\\')
    {
        return Err("invalid external URL".into());
    }
    let decoded = percent_encoding::percent_decode_str(input)
        .decode_utf8()
        .map_err(err)?;
    if decoded.chars().any(char::is_control) {
        return Err("control characters in URL".into());
    }
    let url = url::Url::parse(input).map_err(err)?;
    match url.scheme() {
        "http" | "https"
            if url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none() =>
        {
            Ok(url)
        }
        "mailto"
            if !url.path().is_empty()
                && !url.path().starts_with('/')
                && url.host_str().is_none() =>
        {
            Ok(url)
        }
        _ => Err("only HTTP, HTTPS and mailto links are allowed".into()),
    }
}

mod commands;
pub use commands::run;
