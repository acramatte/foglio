use super::*;
use tauri::{Emitter, Manager, State};

#[derive(Default)]
struct CloseState {
    finishing: AtomicBool,
    finished: AtomicBool,
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T> + Send + 'static,
) -> Result<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(err)?
}
#[tauri::command]
fn desktop_state(backend: State<'_, Arc<Backend>>) -> DesktopState {
    backend.state()
}
#[tauri::command]
async fn select_library(backend: State<'_, Arc<Backend>>, path: String) -> Result<DesktopState> {
    let backend = backend.inner().clone();
    blocking(move || backend.select(Path::new(&path))).await
}
#[tauri::command]
async fn browse_library(backend: State<'_, Arc<Backend>>, session: u64) -> Result<Browse> {
    let backend = backend.inner().clone();
    blocking(move || backend.browse(session)).await
}
#[tauri::command]
async fn search_notes(
    backend: State<'_, Arc<Backend>>,
    session: u64,
    query: String,
    tag: Option<String>,
    folder: Option<String>,
) -> Result<Search> {
    let backend = backend.inner().clone();
    blocking(move || backend.search(session, query, tag, folder)).await
}
#[tauri::command]
async fn open_note(backend: State<'_, Arc<Backend>>, session: u64, path: String) -> Result<Note> {
    let backend = backend.inner().clone();
    blocking(move || backend.open(session, &path)).await
}
async fn mutation_blocking(
    work: impl FnOnce() -> Result<Mutation> + Send + 'static,
) -> Result<Mutation> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| mutation_error("internal", e))?
}
#[tauri::command]
async fn save_note(
    backend: State<'_, Arc<Backend>>,
    session: u64,
    path: String,
    revision: String,
    body: String,
) -> Result<Mutation> {
    let backend = backend.inner().clone();
    mutation_blocking(move || backend.save(session, &path, &revision, &body)).await
}
#[tauri::command]
async fn save_note_copy(
    backend: State<'_, Arc<Backend>>,
    session: u64,
    path: String,
    observed_revision: Option<String>,
    destination: String,
    base_source: String,
    body: String,
) -> Result<Mutation> {
    let backend = backend.inner().clone();
    mutation_blocking(move || {
        backend.save_copy(
            session,
            &path,
            observed_revision.as_deref(),
            &destination,
            &base_source,
            &body,
        )
    })
    .await
}
#[tauri::command]
async fn create_note(
    backend: State<'_, Arc<Backend>>,
    session: u64,
    title: String,
    folder: Option<String>,
    body: String,
    tags: Vec<String>,
) -> Result<Mutation> {
    let backend = backend.inner().clone();
    mutation_blocking(move || {
        backend.create_from_title(session, &title, folder.as_deref(), &body, &tags)
    })
    .await
}
#[tauri::command]
async fn move_note(
    backend: State<'_, Arc<Backend>>,
    session: u64,
    path: String,
    revision: String,
    destination: String,
) -> Result<Mutation> {
    let backend = backend.inner().clone();
    mutation_blocking(move || backend.move_note(session, &path, &revision, &destination)).await
}
#[tauri::command]
async fn delete_note(
    backend: State<'_, Arc<Backend>>,
    session: u64,
    path: String,
    revision: String,
) -> Result<Mutation> {
    let backend = backend.inner().clone();
    mutation_blocking(move || backend.delete(session, &path, &revision)).await
}
#[tauri::command]
async fn change_tag(
    backend: State<'_, Arc<Backend>>,
    session: u64,
    path: String,
    revision: String,
    tag: String,
    add: bool,
) -> Result<Mutation> {
    let backend = backend.inner().clone();
    mutation_blocking(move || backend.change_tag(session, &path, &revision, &tag, add)).await
}
/// Only the frontend's successful flush/discard decision may authorize exit.
#[tauri::command]
async fn finish_close(
    app: tauri::AppHandle,
    backend: State<'_, Arc<Backend>>,
    close: State<'_, Arc<CloseState>>,
) -> Result<()> {
    if close.finishing.swap(true, Ordering::AcqRel) {
        return Ok(());
    }
    let backend = backend.inner().clone();
    let close = close.inner().clone();
    let result = tauri::async_runtime::spawn_blocking(move || backend.shutdown()).await;
    if let Err(error) = result {
        close.finishing.store(false, Ordering::Release);
        return Err(mutation_error("internal", error));
    }
    close.finished.store(true, Ordering::Release);
    app.exit(0);
    Ok(())
}
#[tauri::command]
async fn resolve_note_link(
    backend: State<'_, Arc<Backend>>,
    session: u64,
    from_path: String,
    target: String,
) -> Result<Resolved> {
    let backend = backend.inner().clone();
    blocking(move || backend.resolve_link(session, &from_path, &target)).await
}
#[tauri::command]
async fn open_external_link(url: String) -> Result<()> {
    let url = external_url(&url)?;
    blocking(move || open::that(url.as_str()).map_err(err)).await
}
pub fn run() {
    // Setup only allocates locks and starts an idle subscription drainer; all scans
    // and config access happen in spawn_blocking, including startup and shutdown.
    let backend = Arc::new(Backend::new().expect("desktop event worker"));
    let startup = backend.clone();
    let app = tauri::Builder::default()
        .manage(backend.clone())
        .manage(Arc::new(CloseState::default()))
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event
                && !window
                    .state::<Arc<CloseState>>()
                    .finished
                    .load(Ordering::Acquire)
            {
                api.prevent_close();
                let _ = window.emit("close-requested", ());
            }
        })
        .setup(move |_| {
            tauri::async_runtime::spawn_blocking(move || startup.bootstrap());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            desktop_state,
            select_library,
            browse_library,
            search_notes,
            open_note,
            save_note,
            save_note_copy,
            create_note,
            move_note,
            delete_note,
            change_tag,
            finish_close,
            resolve_note_link,
            open_external_link
        ])
        .build(tauri::generate_context!())
        .expect("build Foglio desktop");
    app.run(move |app, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            if app
                .state::<Arc<CloseState>>()
                .finished
                .load(Ordering::Acquire)
            {
                return;
            }
            api.prevent_exit();
            let _ = app.emit("close-requested", ());
        }
    });
}
