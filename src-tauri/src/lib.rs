#[tauri::command]
fn greet(name: String) -> String {
    format!("Hi {name}, welcome to fedit!")
}

#[tauri::command]
fn echo(msg: String) -> String {
    format!("you said: {msg}")
}

#[tauri::command]
fn read_file(path: String) -> Result<String, String> {
    let contents = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    Ok(contents)
}

#[tauri::command]
fn write_file(path: String, contents: String) -> Result<(), String> {
    std::fs::write(&path, &contents).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![greet, echo, read_file, write_file])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
