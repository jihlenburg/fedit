use std::path::PathBuf;
use std::sync::Mutex;
use serde::{Deserialize, Serialize};
use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use tauri_plugin_store::StoreExt;
use thiserror::Error;

const STORE_FILE: &str = "fedit.json";
const STORE_KEY: &str = "fedit";
const MAX_RECENT: usize = 10;

#[derive(Debug, Error)]
enum FeditError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("no file is open")]
    NoPath,
    #[error("state is poisoned")]
    Poisoned,
}

impl serde::Serialize for FeditError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let kind = match self {
            FeditError::Io(_) => "Io",
            FeditError::Serde(_) => "Serde",
            FeditError::NoPath => "NoPath",
            FeditError::Poisoned => "Poisoned",
        };
        let mut s = serializer.serialize_struct("FeditError", 2)?;
        s.serialize_field("kind", kind)?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

type Result<T> = std::result::Result<T, FeditError>;

#[derive(Default, Serialize, Deserialize)]
struct Persisted {
    #[serde(default)]
    recent: Vec<PathBuf>,
}

#[derive(Default)]
struct AppState {
    current_path: Option<String>,
    is_dirty: bool,
    recent: Vec<PathBuf>,
}

impl AppState {
    fn set_dirty(&mut self, dirty: bool) {
        self.is_dirty = dirty;
    }

    fn remember(&mut self, path: String) {
        self.current_path = Some(path);
        self.is_dirty = false;
    }

    fn push_recent(&mut self, path: PathBuf) {
        self.recent.retain(|p| p != &path);
        self.recent.insert(0, path);
        self.recent = self.recent.iter().take(MAX_RECENT).cloned().collect();
    }
}

fn lock<'a>(state: &'a State<Mutex<AppState>>) -> Result<std::sync::MutexGuard<'a, AppState>> {
    state.lock().map_err(|_| FeditError::Poisoned)
}

fn load_persisted(app: &AppHandle) -> Persisted {
    let Ok(store) = app.store(STORE_FILE) else {
        return Persisted::default();
    };
    match store.get(STORE_KEY) {
        Some(val) => serde_json::from_value(val).unwrap_or_default(),
        None => Persisted::default(),
    }
}

fn save_persisted(app: &AppHandle, persisted: &Persisted) {
    let Ok(store) = app.store(STORE_FILE) else {
        return;
    };
    if let Ok(val) = serde_json::to_value(persisted) {
        store.set(STORE_KEY, val);
        let _ = store.save();
    }
}

fn persist_recent(app: &AppHandle, recent: &[PathBuf]) {
    save_persisted(app, &Persisted { recent: recent.to_vec() });
}

#[tauri::command]
fn greet(name: String) -> String {
    format!("Hi {name}, welcome to fedit!")
}

#[tauri::command]
fn echo(msg: String) -> String {
    format!("you said: {msg}")
}

#[tauri::command]
fn read_file(
    path: String,
    state: State<Mutex<AppState>>,
    app: AppHandle,
) -> Result<String> {
    let contents = std::fs::read_to_string(&path)?;
    let mut s = lock(&state)?;
    s.remember(path.clone());
    s.push_recent(PathBuf::from(path));
    persist_recent(&app, &s.recent);
    Ok(contents)
}

#[tauri::command]
fn save(
    new_path: Option<String>,
    contents: String,
    state: State<Mutex<AppState>>,
    app: AppHandle,
) -> Result<String> {
    let mut s = lock(&state)?;
    let path = match new_path {
        Some(p) => p,
        None => match &s.current_path {
            Some(p) => p.clone(),
            None => return Err(FeditError::NoPath),
        },
    };
    std::fs::write(&path, &contents)?;
    s.remember(path.clone());
    s.push_recent(PathBuf::from(&path));
    persist_recent(&app, &s.recent);
    Ok(path)
}

#[tauri::command]
fn current_path(state: State<Mutex<AppState>>) -> Result<Option<String>> {
    let s = lock(&state)?;
    Ok(s.current_path.clone())
}

#[tauri::command]
fn set_dirty(dirty: bool, state: State<Mutex<AppState>>) -> Result<()> {
    let mut s = lock(&state)?;
    s.set_dirty(dirty);
    Ok(())
}

#[tauri::command]
fn is_dirty(state: State<Mutex<AppState>>) -> Result<bool> {
    let s = lock(&state)?;
    Ok(s.is_dirty)
}

#[tauri::command]
fn recent_files(state: State<Mutex<AppState>>) -> Result<Vec<PathBuf>> {
    let s = lock(&state)?;
    Ok(s.recent.clone())
}

fn build_menu(app: &AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let new_item = MenuItemBuilder::new("New")
        .id("new")
        .accelerator("CmdOrCtrl+N")
        .build(app)?;
    let open_item = MenuItemBuilder::new("Open…")
        .id("open")
        .accelerator("CmdOrCtrl+O")
        .build(app)?;
    let save_item = MenuItemBuilder::new("Save")
        .id("save")
        .accelerator("CmdOrCtrl+S")
        .build(app)?;
    let save_as_item = MenuItemBuilder::new("Save As…")
        .id("save-as")
        .accelerator("CmdOrCtrl+Shift+S")
        .build(app)?;

    let file_menu = SubmenuBuilder::new(app, "File")
        .item(&new_item)
        .separator()
        .item(&open_item)
        .item(&save_item)
        .item(&save_as_item)
        .separator()
        .item(&PredefinedMenuItem::quit(app, None)?)
        .build()?;

    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .item(&PredefinedMenuItem::undo(app, None)?)
        .item(&PredefinedMenuItem::redo(app, None)?)
        .separator()
        .item(&PredefinedMenuItem::cut(app, None)?)
        .item(&PredefinedMenuItem::copy(app, None)?)
        .item(&PredefinedMenuItem::paste(app, None)?)
        .item(&PredefinedMenuItem::select_all(app, None)?)
        .build()?;

    MenuBuilder::new(app).item(&file_menu).item(&edit_menu).build()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .manage(Mutex::new(AppState::default()))
        .invoke_handler(tauri::generate_handler![
            greet,
            echo,
            read_file,
            save,
            current_path,
            set_dirty,
            is_dirty,
            recent_files,
        ])
        .setup(|app| {
            let persisted = load_persisted(&app.handle().clone());
            {
                let state: State<Mutex<AppState>> = app.state();
                if let Ok(mut s) = state.lock() {
                    s.recent = persisted.recent;
                }
            }

            let menu = build_menu(&app.handle().clone())?;
            app.set_menu(menu)?;
            app.on_menu_event(|app_handle, event| {
                if let Some(window) = app_handle.get_webview_window("main") {
                    let id = event.id().as_ref();
                    let _ = window.emit(&format!("fedit:menu-{id}"), ());
                }
            });

            let window = app.get_webview_window("main").unwrap();
            let handle = app.handle().clone();
            let window_for_emit = window.clone();
            window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    let state: State<Mutex<AppState>> = handle.state();
                    let dirty = state.lock().map(|s| s.is_dirty).unwrap_or(false);
                    if dirty {
                        api.prevent_close();
                        let _ = window_for_emit.emit("fedit:close-blocked", ());
                    }
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
