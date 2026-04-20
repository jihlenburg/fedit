use std::sync::Mutex;
use tauri::{Emitter, Manager, State, WindowEvent};
use thiserror::Error;

#[derive(Debug, Error)]
enum FeditError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
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

#[derive(Default)]
struct AppState {
    current_path: Option<String>,
    is_dirty: bool,
}

impl AppState {
    fn set_dirty(&mut self, dirty: bool) {
        self.is_dirty = dirty;
    }

    fn remember(&mut self, path: String) {
        self.current_path = Some(path);
        self.is_dirty = false;
    }
}

fn lock<'a>(state: &'a State<Mutex<AppState>>) -> Result<std::sync::MutexGuard<'a, AppState>> {
    state.lock().map_err(|_| FeditError::Poisoned)
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
fn read_file(path: String, state: State<Mutex<AppState>>) -> Result<String> {
    let contents = std::fs::read_to_string(&path)?;
    let mut s = lock(&state)?;
    s.remember(path);
    Ok(contents)
}

#[tauri::command]
fn save(
    new_path: Option<String>,
    contents: String,
    state: State<Mutex<AppState>>,
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Mutex::new(AppState::default()))
        .invoke_handler(tauri::generate_handler![
            greet,
            echo,
            read_file,
            save,
            current_path,
            set_dirty,
            is_dirty,
        ])
        .setup(|app| {
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
