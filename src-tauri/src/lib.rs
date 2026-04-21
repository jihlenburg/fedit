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
    #[error("bad regex: {0}")]
    Regex(#[from] regex::Error),
    #[error("no file is open")]
    NoPath,
    #[error("no such tab")]
    NoTab,
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
            FeditError::Regex(_) => "Regex",
            FeditError::NoPath => "NoPath",
            FeditError::NoTab => "NoTab",
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

/// One open document. `path: None` means an untitled buffer that hasn't been
/// saved yet — the "new tab" state. `dirty` is the editor's unsaved-changes
/// flag, mirrored from the frontend on every keystroke.
#[derive(Clone, Serialize)]
struct Tab {
    id: u64,
    path: Option<String>,
    dirty: bool,
}

#[derive(Default)]
struct AppState {
    tabs: Vec<Tab>,
    /// Index into `tabs`. `None` only when `tabs` is empty.
    active: Option<usize>,
    next_id: u64,
    recent: Vec<PathBuf>,
    /// When true, the close-window handler stops guarding on dirty tabs —
    /// set by the frontend after the user confirms the "discard unsaved?"
    /// dialog, then cleared after the window closes.
    force_close: bool,
}

impl AppState {
    fn bump_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    fn push_recent(&mut self, path: PathBuf) {
        self.recent.retain(|p| p != &path);
        self.recent.insert(0, path);
        self.recent = self.recent.iter().take(MAX_RECENT).cloned().collect();
    }

    fn find_index(&self, id: u64) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == id)
    }

    fn find_path_index(&self, path: &str) -> Option<usize> {
        self.tabs.iter().position(|t| t.path.as_deref() == Some(path))
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

/// Create a new untitled tab and make it active. Returns the new tab so the
/// frontend can track its id.
#[tauri::command]
fn new_tab(state: State<Mutex<AppState>>) -> Result<Tab> {
    let mut s = lock(&state)?;
    let id = s.bump_id();
    let tab = Tab { id, path: None, dirty: false };
    s.tabs.push(tab.clone());
    s.active = Some(s.tabs.len() - 1);
    Ok(tab)
}

/// The payload returned by `open_tab`: backend tab metadata + file contents.
/// `already_open` tells the frontend whether we created a new tab or switched
/// to an existing one — so it knows whether to reuse the cached buffer.
#[derive(Serialize)]
struct OpenedTab {
    tab: Tab,
    contents: String,
    already_open: bool,
}

/// Open a file in a new tab — or switch to the existing tab if that path is
/// already open. The frontend uses `already_open` to decide whether to trust
/// its cached editor buffer (e.g. after the user edited but didn't save) or
/// replace it with the on-disk contents.
#[tauri::command]
fn open_tab(
    path: String,
    state: State<Mutex<AppState>>,
    app: AppHandle,
) -> Result<OpenedTab> {
    let contents = std::fs::read_to_string(&path)?;
    let mut s = lock(&state)?;
    if let Some(idx) = s.find_path_index(&path) {
        s.active = Some(idx);
        let tab = s.tabs[idx].clone();
        s.push_recent(PathBuf::from(&path));
        persist_recent(&app, &s.recent);
        return Ok(OpenedTab { tab, contents, already_open: true });
    }
    let id = s.bump_id();
    let tab = Tab { id, path: Some(path.clone()), dirty: false };
    s.tabs.push(tab.clone());
    s.active = Some(s.tabs.len() - 1);
    s.push_recent(PathBuf::from(path));
    persist_recent(&app, &s.recent);
    Ok(OpenedTab { tab, contents, already_open: false })
}

/// Close a tab by id. Adjusts `active` so it keeps pointing at a valid tab
/// (or becomes `None` if every tab is now closed). Returns the remaining tab
/// list so the frontend can re-render without a separate round-trip.
#[tauri::command]
fn close_tab(id: u64, state: State<Mutex<AppState>>) -> Result<Vec<Tab>> {
    let mut s = lock(&state)?;
    let Some(idx) = s.find_index(id) else {
        return Ok(s.tabs.clone());
    };
    s.tabs.remove(idx);
    s.active = match s.active {
        None => None,
        Some(a) if s.tabs.is_empty() => {
            let _ = a;
            None
        }
        Some(a) if a == idx => Some(a.min(s.tabs.len() - 1)),
        Some(a) if a > idx => Some(a - 1),
        other => other,
    };
    Ok(s.tabs.clone())
}

#[tauri::command]
fn switch_tab(id: u64, state: State<Mutex<AppState>>) -> Result<()> {
    let mut s = lock(&state)?;
    if let Some(idx) = s.find_index(id) {
        s.active = Some(idx);
        Ok(())
    } else {
        Err(FeditError::NoTab)
    }
}

#[tauri::command]
fn list_tabs(state: State<Mutex<AppState>>) -> Result<Vec<Tab>> {
    let s = lock(&state)?;
    Ok(s.tabs.clone())
}

#[tauri::command]
fn active_tab(state: State<Mutex<AppState>>) -> Result<Option<Tab>> {
    let s = lock(&state)?;
    Ok(s.active.map(|i| s.tabs[i].clone()))
}

#[tauri::command]
fn set_tab_dirty(
    id: u64,
    dirty: bool,
    state: State<Mutex<AppState>>,
) -> Result<()> {
    let mut s = lock(&state)?;
    match s.tabs.iter_mut().find(|t| t.id == id) {
        Some(t) => {
            t.dirty = dirty;
            Ok(())
        }
        None => Err(FeditError::NoTab),
    }
}

/// True if any tab has unsaved changes — the close-window handler uses this
/// to decide whether to block the close.
#[tauri::command]
fn any_dirty(state: State<Mutex<AppState>>) -> Result<bool> {
    let s = lock(&state)?;
    Ok(s.tabs.iter().any(|t| t.dirty))
}

/// Set the force-close flag and ask the window to close again. The window-event
/// handler sees the flag and lets the close through this time. Used by the
/// frontend after the user confirms "discard unsaved changes".
#[tauri::command]
fn force_close_window(
    state: State<Mutex<AppState>>,
    window: tauri::Window,
) -> Result<()> {
    {
        let mut s = lock(&state)?;
        s.force_close = true;
    }
    let _ = window.close();
    Ok(())
}

#[tauri::command]
fn save(
    id: u64,
    new_path: Option<String>,
    contents: String,
    state: State<Mutex<AppState>>,
    app: AppHandle,
) -> Result<String> {
    let mut s = lock(&state)?;
    let idx = s.find_index(id).ok_or(FeditError::NoTab)?;
    let path = match new_path {
        Some(p) => p,
        None => match &s.tabs[idx].path {
            Some(p) => p.clone(),
            None => return Err(FeditError::NoPath),
        },
    };
    std::fs::write(&path, &contents)?;
    s.tabs[idx].path = Some(path.clone());
    s.tabs[idx].dirty = false;
    s.push_recent(PathBuf::from(&path));
    persist_recent(&app, &s.recent);
    Ok(path)
}

#[tauri::command]
fn recent_files(state: State<Mutex<AppState>>) -> Result<Vec<PathBuf>> {
    let s = lock(&state)?;
    Ok(s.recent.clone())
}

/// A single match in the buffer: start/end are BYTE offsets into `haystack`.
/// JS converts these to a selection range via textarea.setSelectionRange.
#[derive(Serialize)]
struct Match {
    start: usize,
    end: usize,
}

/// Find every match of `needle` in `haystack`. If `use_regex` is true, `needle`
/// is compiled as a regex; otherwise the literal bytes are searched. A `case_sensitive`
/// flag flips `(?i)` on/off in the compiled regex.
#[tauri::command]
fn find_matches(
    haystack: String,
    needle: String,
    use_regex: bool,
    case_sensitive: bool,
) -> Result<Vec<Match>> {
    if needle.is_empty() {
        return Ok(vec![]);
    }
    let pattern = if use_regex {
        needle.clone()
    } else {
        regex::escape(&needle)
    };
    let pattern = if case_sensitive {
        pattern
    } else {
        format!("(?i){pattern}")
    };
    let re = regex::Regex::new(&pattern)?;
    Ok(re
        .find_iter(&haystack)
        .map(|m| Match { start: m.start(), end: m.end() })
        .collect())
}

/// Replace every match of `needle` in `haystack`. Returns the new string.
/// JS assigns it to editor.value — the textarea doesn't need to know about regex.
#[tauri::command]
fn replace_matches(
    haystack: String,
    needle: String,
    replacement: String,
    use_regex: bool,
    case_sensitive: bool,
) -> Result<String> {
    if needle.is_empty() {
        return Ok(haystack);
    }
    let pattern = if use_regex {
        needle.clone()
    } else {
        regex::escape(&needle)
    };
    let pattern = if case_sensitive {
        pattern
    } else {
        format!("(?i){pattern}")
    };
    let re = regex::Regex::new(&pattern)?;
    Ok(re.replace_all(&haystack, replacement.as_str()).into_owned())
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
    let close_tab_item = MenuItemBuilder::new("Close Tab")
        .id("close-tab")
        .accelerator("CmdOrCtrl+W")
        .build(app)?;

    let file_menu = SubmenuBuilder::new(app, "File")
        .item(&new_item)
        .separator()
        .item(&open_item)
        .item(&save_item)
        .item(&save_as_item)
        .separator()
        .item(&close_tab_item)
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
            new_tab,
            open_tab,
            close_tab,
            switch_tab,
            list_tabs,
            active_tab,
            set_tab_dirty,
            any_dirty,
            force_close_window,
            save,
            recent_files,
            find_matches,
            replace_matches,
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
                    // Two flags guard this branch: `force_close` means the
                    // frontend already confirmed a discard, so let the close
                    // through and clear the flag. Otherwise, prevent the close
                    // if any tab is dirty and ask the frontend to show a
                    // confirm dialog.
                    let (force, dirty) = state
                        .lock()
                        .map(|mut s| {
                            let force = s.force_close;
                            if force { s.force_close = false; }
                            (force, s.tabs.iter().any(|t| t.dirty))
                        })
                        .unwrap_or((false, false));
                    if !force && dirty {
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
