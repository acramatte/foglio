//! Linux in-process observation; filesystem hints never authorize mutations.
//!
//! Batches conservatively reconcile the entire library (including subtree changes)
//! and hash all content. This favors identity/unknown-coverage correctness over
//! large-library throughput. No database connection or library lock is held idle.
use crate::{
    Error, ErrorCode, Library, Result,
    events::{Event, EventKind, WatchedNote},
    index::IndexStatus,
};
use notify::{Config, EventKind as NativeKind, RecommendedWatcher, RecursiveMode, Watcher as _};
use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    path::PathBuf,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
#[cfg(test)]
#[path = "watcher_tests.rs"]
mod tests;

const MANUAL: usize = 1;
const LOST: usize = 2;
const CLIENT: usize = 4;
const RESUME: usize = 8;
const TICK: Duration = Duration::from_millis(10);

#[derive(Debug, Clone)]
pub struct WatchOptions {
    /// Quiet period; max_batch_delay prevents starvation under continuous writes.
    pub debounce: Duration,
    pub max_batch_delay: Duration,
    /// Forced content reconciliation even when the backend delivers no hints.
    pub safety_interval: Duration,
    pub hint_capacity: usize,
    pub dirty_capacity: usize,
}
impl Default for WatchOptions {
    fn default() -> Self {
        Self {
            debounce: Duration::from_millis(75),
            max_batch_delay: Duration::from_millis(500),
            safety_interval: Duration::from_secs(30),
            hint_capacity: 1024,
            dirty_capacity: 1024,
        }
    }
}
impl WatchOptions {
    fn validate(&self) -> Result<()> {
        if self.debounce.is_zero()
            || self.max_batch_delay < self.debounce
            || self.max_batch_delay > Duration::from_secs(60)
            || self.safety_interval < TICK
            || self.safety_interval > Duration::from_secs(3600)
            || !(1..=65_536).contains(&self.hint_capacity)
            || !(1..=65_536).contains(&self.dirty_capacity)
        {
            return Err(Error::new(
                ErrorCode::Usage,
                "invalid watcher intervals or queue capacities",
            ));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Default)]
pub struct WatchSnapshot {
    /// Monotonic for this watcher lifetime, advanced after each reconciliation.
    /// Event gaps require refetch; this is not a persisted/distributed sequence.
    pub generation: u64,
    /// Last committed eligible records, potentially stale when error is present.
    pub notes: Vec<WatchedNote>,
    pub status: IndexStatus,
    pub error: Option<String>,
    pub paused: bool,
}
struct Queue {
    events: VecDeque<Event>,
    capacity: usize,
    invalidated: bool,
    closed: bool,
}
/// A bounded nonblocking queue. Consume the initial invalidation, then refetch
/// the snapshot. EVERY invalidation requires another refetch, even if a snapshot
/// was just obtained: deltas are suppressed until the invalidation is consumed.
pub struct Subscription {
    queue: Arc<Mutex<Queue>>,
}
impl Subscription {
    pub fn try_recv(&self) -> Option<Event> {
        let mut q = self.queue.lock().unwrap();
        let event = q.events.pop_front();
        if event.is_some() {
            q.invalidated = false;
        }
        event
    }
    pub fn is_closed(&self) -> bool {
        self.queue.lock().unwrap().closed
    }
}
struct Shared {
    snapshot: Mutex<WatchSnapshot>,
    subscribers: Mutex<Vec<Weak<Mutex<Queue>>>>,
    recovery: AtomicUsize,
    stopped: AtomicBool,
    paused: AtomicBool,
}
impl Shared {
    fn publish(&self, generation: u64, kind: EventKind) {
        let mut subscribers = self.subscribers.lock().unwrap();
        subscribers.retain(|weak| {
            let Some(queue) = weak.upgrade() else {
                return false;
            };
            let mut q = queue.lock().unwrap();
            if q.invalidated {
                return true;
            }
            if q.events.len() == q.capacity {
                q.events.clear();
                q.invalidated = true;
                q.events.push_back(Event {
                    generation,
                    kind: EventKind::RescanRequired {
                        reason: "subscriber_overflow".into(),
                    },
                });
                self.recovery.fetch_or(CLIENT, Ordering::Release);
            } else {
                q.events.push_back(Event {
                    generation,
                    kind: kind.clone(),
                });
            }
            true
        });
    }
    fn invalidate(&self, reason: &str) {
        let generation = self.snapshot.lock().unwrap().generation;
        self.publish(
            generation,
            EventKind::RescanRequired {
                reason: reason.into(),
            },
        );
    }
    fn finish(&self) {
        self.snapshot.lock().unwrap().status.watcher_active = false;
        let subscribers = self.subscribers.lock().unwrap();
        for q in subscribers.iter().filter_map(Weak::upgrade) {
            q.lock().unwrap().closed = true;
        }
    }
}
/// Owns the native backend and worker lifetime. Drop joins the worker; shutdown
/// additionally reports panics. In-flight filesystem/SQLite work is not cancelled.
pub struct Watcher {
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}
impl Watcher {
    pub fn start(library: Arc<Library>, options: WatchOptions) -> Result<Self> {
        options.validate()?;
        let shared = Arc::new(Shared {
            snapshot: Mutex::new(WatchSnapshot::default()),
            subscribers: Mutex::new(Vec::new()),
            recovery: AtomicUsize::new(0),
            stopped: AtomicBool::new(false),
            paused: AtomicBool::new(false),
        });
        let (tx, rx) = mpsc::sync_channel(options.hint_capacity);
        // Register BEFORE any scan. Callback is bounded/nonblocking during startup.
        let native = observe(&library, tx.clone(), shared.clone(), options.dirty_capacity)?;
        reconcile(&library, &shared, None);
        let state = shared.clone();
        let worker = thread::Builder::new()
            .name("foglio-watcher".into())
            .spawn(move || {
                run(library, options, state.clone(), tx, rx, native);
                state.finish();
            })?;
        Ok(Self {
            shared,
            worker: Some(worker),
        })
    }
    pub fn snapshot(&self) -> WatchSnapshot {
        self.shared.snapshot.lock().unwrap().clone()
    }
    pub fn subscribe(&self, capacity: usize) -> Result<Subscription> {
        if !(1..=65_536).contains(&capacity) {
            return Err(Error::new(
                ErrorCode::Usage,
                "subscription capacity must be 1..=65536",
            ));
        }
        // Same subscribers->snapshot ordering as publish; registration/refetch gap
        // is covered by the initial sticky invalidation.
        let mut subscribers = self.shared.subscribers.lock().unwrap();
        subscribers.retain(|s| s.strong_count() != 0);
        if subscribers.len() >= 1024 {
            return Err(Error::new(ErrorCode::Busy, "too many subscriptions"));
        }
        let generation = self.snapshot().generation;
        let queue = Arc::new(Mutex::new(Queue {
            events: VecDeque::from([Event {
                generation,
                kind: EventKind::RescanRequired {
                    reason: "subscribed".into(),
                },
            }]),
            capacity,
            invalidated: true,
            closed: false,
        }));
        subscribers.push(Arc::downgrade(&queue));
        Ok(Subscription { queue })
    }
    /// Request asynchronous full content reconciliation (no adoption).
    pub fn rescan(&self) {
        self.shared.recovery.fetch_or(MANUAL, Ordering::Release);
    }
    /// Report an upstream gap/overflow, including when an embedding drops hints.
    pub fn report_event_loss(&self) {
        self.shared.recovery.fetch_or(LOST, Ordering::Release);
    }
    /// Pause reconciliation, intentionally discarding hints. Resume always rescans.
    pub fn pause(&self) {
        self.shared.paused.store(true, Ordering::Release);
        self.shared.snapshot.lock().unwrap().paused = true;
    }
    pub fn resume(&self) {
        self.shared.recovery.fetch_or(RESUME, Ordering::Release);
        self.shared.paused.store(false, Ordering::Release);
    }
    pub fn shutdown(mut self) -> Result<()> {
        self.join()
    }
    fn join(&mut self) -> Result<()> {
        self.shared.stopped.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            if worker.join().is_err() {
                self.shared.finish();
                return Err(Error::new(ErrorCode::Io, "watcher worker panicked"));
            }
        }
        Ok(())
    }
}
impl Drop for Watcher {
    fn drop(&mut self) {
        let _ = self.join();
    }
}

struct RegisteredWatcher {
    _native: RecommendedWatcher,
    library: Arc<Library>,
}
impl Drop for RegisteredWatcher {
    fn drop(&mut self) {
        self.library.watcher_backends.fetch_sub(1, Ordering::AcqRel);
    }
}

fn observe(
    library: &Arc<Library>,
    tx: SyncSender<Vec<PathBuf>>,
    shared: Arc<Shared>,
    limit: usize,
) -> Result<RegisteredWatcher> {
    let root = library.root();
    crate::filesystem::check_chain(root)?;
    let mut native = RecommendedWatcher::new(
        move |result: notify::Result<notify::Event>| {
            if shared.paused.load(Ordering::Acquire) {
                return;
            }
            match result {
                Ok(event) => {
                    if event.need_rescan() {
                        shared.recovery.fetch_or(LOST, Ordering::Release);
                    }
                    // Reads from our scanner generate access hints. Ignoring access is
                    // essential; modify/metadata/rename/create/remove are never suppressed.
                    if matches!(event.kind, NativeKind::Access(_)) {
                        return;
                    }
                    if event.paths.is_empty()
                        || event.paths.len() > limit
                        || matches!(event.kind, NativeKind::Any | NativeKind::Other)
                    {
                        shared.recovery.fetch_or(LOST, Ordering::Release);
                        return;
                    }
                    if tx.try_send(event.paths).is_err() {
                        shared.recovery.fetch_or(LOST, Ordering::Release);
                    }
                }
                Err(_) => {
                    shared.recovery.fetch_or(LOST, Ordering::Release);
                }
            }
        },
        Config::default().with_follow_symlinks(false),
    )
    .map_err(|e| Error::new(ErrorCode::Io, e))?;
    native
        .watch(root, RecursiveMode::Recursive)
        .map_err(|e| Error::new(ErrorCode::Io, e))?;
    library.watcher_backends.fetch_add(1, Ordering::AcqRel);
    Ok(RegisteredWatcher {
        _native: native,
        library: library.clone(),
    })
}
fn run(
    library: Arc<Library>,
    options: WatchOptions,
    shared: Arc<Shared>,
    tx: SyncSender<Vec<PathBuf>>,
    rx: Receiver<Vec<PathBuf>>,
    native: RegisteredWatcher,
) {
    let mut native = Some(native);
    let mut dirty = HashSet::new();
    let mut first = None;
    let mut last = Instant::now();
    let mut scanned = Instant::now();
    let mut backend_error = None;
    while !shared.stopped.load(Ordering::Acquire) {
        let paused = shared.paused.load(Ordering::Acquire);
        // Bound work per turn so a continuous producer cannot starve recovery/stop.
        for _ in 0..options.hint_capacity {
            let Ok(paths) = rx.try_recv() else {
                break;
            };
            if paused {
                continue;
            }
            let now = Instant::now();
            first.get_or_insert(now);
            last = now;
            for path in paths {
                if path == library.root() || !path.starts_with(library.root()) {
                    shared.recovery.fetch_or(LOST, Ordering::Release);
                }
                if dirty.len() < options.dirty_capacity {
                    dirty.insert(path);
                } else {
                    shared.recovery.fetch_or(LOST, Ordering::Release);
                }
            }
        }
        if paused {
            dirty.clear();
            first = None;
            thread::park_timeout(TICK);
            continue;
        }
        let recovery = shared.recovery.swap(0, Ordering::AcqRel);
        let periodic = scanned.elapsed() >= options.safety_interval;
        let retry = backend_error.is_some() && scanned.elapsed() >= Duration::from_secs(1);
        let due = first.is_some_and(|at: Instant| {
            at.elapsed() >= options.max_batch_delay || last.elapsed() >= options.debounce
        });
        if recovery != 0 || periodic || due || retry {
            if recovery != 0 {
                shared.invalidate(if recovery & LOST != 0 {
                    "watcher_gap"
                } else if recovery & CLIENT != 0 {
                    "subscriber_gap"
                } else if recovery & RESUME != 0 {
                    "resumed"
                } else {
                    "manual"
                });
            }
            // Re-register after gaps/root moves, and retry missing/unreadable roots.
            // Observe before reconciliation again, not after it.
            if recovery & (LOST | RESUME) != 0 || backend_error.is_some() {
                native.take();
                match observe(&library, tx.clone(), shared.clone(), options.dirty_capacity) {
                    Ok(w) => {
                        native = Some(w);
                        backend_error = None;
                    }
                    Err(_) => {
                        backend_error =
                            Some("native watcher unavailable; retrying registration".to_string());
                    }
                }
            }
            reconcile(&library, &shared, backend_error.clone());
            dirty.clear();
            first = None;
            scanned = Instant::now();
        }
        thread::park_timeout(TICK);
    }
    drop(native);
}
fn reconcile(library: &Library, shared: &Shared, backend_error: Option<String>) {
    let previous = shared.snapshot.lock().unwrap().clone();
    let mut next = previous.clone();
    next.generation += 1;
    next.paused = shared.paused.load(Ordering::Acquire);
    let native_active = backend_error.is_none();
    match library.watch_reconcile() {
        Ok((status, notes)) => {
            next.status = status;
            next.notes = notes;
            next.error = backend_error;
        }
        Err(e) => {
            next.error = Some(e.to_string());
            next.status.incomplete = true;
        }
    }
    next.status.watcher_active = !shared.stopped.load(Ordering::Acquire) && native_active;
    if next.error.is_some() {
        next.status.incomplete = true;
    }
    let generation = next.generation;
    // Release state lock before publishing: subscription registration uses reverse
    // access order, and consumers must be able to refetch committed state immediately.
    *shared.snapshot.lock().unwrap() = next.clone();
    let old_degraded = previous.status.incomplete || previous.error.is_some();
    let degraded = next.status.incomplete || next.error.is_some();
    if previous.generation == 0 || old_degraded != degraded || previous.error != next.error {
        shared.publish(
            generation,
            EventKind::IndexStateChanged {
                degraded,
                error: next.error.clone(),
            },
        );
    }
    if serde_json::to_string(&previous.status.diagnostics).unwrap()
        != serde_json::to_string(&next.status.diagnostics).unwrap()
    {
        shared.publish(
            generation,
            EventKind::DiagnosticsChanged {
                diagnostics: next.status.diagnostics.clone(),
            },
        );
    }
    if degraded || old_degraded {
        if previous.notes != next.notes || old_degraded != degraded || previous.error != next.error
        {
            shared.publish(
                generation,
                EventKind::RescanRequired {
                    reason: "incomplete_or_recovered_identity_map".into(),
                },
            );
        }
        // Eligibility loss is not proof of file deletion. Never invent deletes
        // for unknown coverage, malformed metadata or duplicate conflict copies.
        return;
    }
    let old: BTreeMap<_, _> = previous.notes.iter().map(|n| (n.id.as_str(), n)).collect();
    let new: BTreeMap<_, _> = next.notes.iter().map(|n| (n.id.as_str(), n)).collect();
    for (id, note) in &old {
        if !new.contains_key(id) {
            shared.publish(
                generation,
                EventKind::NoteDeleted {
                    note: (*note).clone(),
                },
            );
        }
    }
    for (id, note) in new {
        let kind = match old.get(id) {
            None => Some(EventKind::NoteCreated { note: note.clone() }),
            Some(before) if before.path != note.path => Some(EventKind::NoteMoved {
                note: note.clone(),
                from: before.path.clone(),
                previous_revision: before.revision.clone(),
            }),
            Some(before) if before.revision != note.revision => Some(EventKind::NoteChanged {
                note: note.clone(),
                previous_revision: before.revision.clone(),
            }),
            _ => None,
        };
        if let Some(kind) = kind {
            shared.publish(generation, kind);
        }
    }
}
