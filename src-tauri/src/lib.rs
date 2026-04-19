#[tauri::command]
fn greet(name: String) -> String {
    format!("Hi {name}, welcome to fedit!")
}

#[tauri::command]
fn echo(msg: String) -> String {
    format!("you said: {msg}")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![greet, echo])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
