use super::*;
use tauri::{Manager, State};

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
async fn open_note(backend: State<'_, Arc<Backend>>, session: u64, id: String) -> Result<Note> {
    let backend = backend.inner().clone();
    blocking(move || backend.open(session, &id)).await
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
            resolve_note_link,
            open_external_link
        ])
        .build(tauri::generate_context!())
        .expect("build Foglio desktop");
    let closing = Arc::new(AtomicBool::new(false));
    let finished = Arc::new(AtomicBool::new(false));
    app.run(move |app, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            if finished.load(Ordering::Acquire) {
                return;
            }
            api.prevent_exit();
            if !closing.swap(true, Ordering::AcqRel) {
                let backend = app.state::<Arc<Backend>>().inner().clone();
                let app = app.clone();
                let finished = finished.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    backend.shutdown();
                    finished.store(true, Ordering::Release);
                    app.exit(0);
                });
            }
        }
    });
}
