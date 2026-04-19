use std::sync::Mutex;
use tauri::State;

#[derive(Default)]
struct AppState {
    current_path: Option<String>,
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
    s.current_path = Some(path);
    Ok(contents)
}

#[tauri::command]
fn write_file(
    path: String,
    contents: String,
    state: State<Mutex<AppState>>,
) -> Result<(), String> {
    std::fs::write(&path, &contents).map_err(|e| e.to_string())?;
    let mut s = state.lock().map_err(|e| e.to_string())?;
    s.current_path = Some(path);
    Ok(())
}

#[tauri::command]
fn current_path(state: State<Mutex<AppState>>) -> Result<Option<String>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    Ok(s.current_path.clone())
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
            write_file,
            current_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
