use std::sync::Mutex;
use tauri::{Emitter, Manager, State, WindowEvent};

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

#[tauri::command]
fn greet(name: String) -> String {
    format!("Hi {name}, welcome to fedit!")
}

#[tauri::command]
fn echo(msg: String) -> String {
    format!("you said: {msg}")
}

#[tauri::command]
fn read_file(path: String, state: State<Mutex<AppState>>) -> Result<String, String> {
    let contents = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.remember(path);
    Ok(contents)
}

#[tauri::command]
fn save(
    new_path: Option<String>,
    contents: String,
    state: State<Mutex<AppState>>,
) -> Result<String, String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    let path = match new_path {
        Some(p) => p,
        None => match &s.current_path {
            Some(p) => p.clone(),
            None => return Err("no-path".to_string()),
        },
    };
    std::fs::write(&path, &contents).map_err(|e| e.to_string())?;
    s.remember(path.clone());
    Ok(path)
}

#[tauri::command]
fn current_path(state: State<Mutex<AppState>>) -> Result<Option<String>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(s.current_path.clone())
}

#[tauri::command]
fn set_dirty(dirty: bool, state: State<Mutex<AppState>>) -> Result<(), String> {
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.set_dirty(dirty);
    Ok(())
}

#[tauri::command]
fn is_dirty(state: State<Mutex<AppState>>) -> Result<bool, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
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
