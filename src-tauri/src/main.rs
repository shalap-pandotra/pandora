// Prevents additional console window on Windows in release, DO NOT REMOVE!!

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod library;

use std::sync::Mutex;
use tauri::State;
use tauri::Manager;
use rusqlite::Connection;

struct AppState {
    library_path: Mutex<Option<String>>,
    conn: Mutex<Option<Connection>>,
}

fn config_path(app: &tauri::AppHandle) -> std::path::PathBuf {
    let dir = app.path().app_config_dir().expect("no config dir");
    std::fs::create_dir_all(&dir).ok();
    dir.join("config.json")
}

#[tauri::command]
fn save_last_library_path(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let json = serde_json::json!({ "last_library_path": path });
    std::fs::write(config_path(&app), json.to_string()).map_err(|e| e.to_string())
}

#[tauri::command]
fn load_last_library_path(app: tauri::AppHandle) -> Option<String> {
    let content = std::fs::read_to_string(config_path(&app)).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&content).ok()?;
    parsed.get("last_library_path")?.as_str().map(String::from)
}

#[tauri::command]
fn set_library_path(path: String, state: State<AppState>) -> Result<(), String> {
    let conn = library::open_library(&path).map_err(|e| e.to_string())?;
    *state.conn.lock().unwrap() = Some(conn);
    *state.library_path.lock().unwrap() = Some(path);
    Ok(())
}

fn get_path(state: &State<AppState>) -> Result<String, String> {
    state.library_path.lock().unwrap().clone().ok_or_else(|| "No library path set".to_string())
}

fn with_conn<T>(state: &State<AppState>, f: impl FnOnce(&Connection) -> Result<T, String>) -> Result<T, String> {
    let guard = state.conn.lock().unwrap();
    let conn = guard.as_ref().ok_or("No library open")?;
    f(conn)
}

#[tauri::command]
fn get_authors(state: State<AppState>) -> Result<Vec<library::FilterRow>, String> {
    with_conn(&state, |conn| library::list_authors(conn).map_err(|e| e.to_string()))
}

#[tauri::command]
fn get_series(state: State<AppState>) -> Result<Vec<library::FilterRow>, String> {
    with_conn(&state, |conn| library::list_series(conn).map_err(|e| e.to_string()))
}

#[tauri::command]
fn get_tags(state: State<AppState>) -> Result<Vec<library::FilterRow>, String> {
    with_conn(&state, |conn| library::list_tags(conn).map_err(|e| e.to_string()))
}

#[tauri::command]
fn get_multi_text_columns(state: State<AppState>) -> Result<Vec<library::MultiTextColumn>, String> {
    with_conn(&state, |conn| library::list_multi_text_columns(conn).map_err(|e| e.to_string()))
}

#[tauri::command]
fn get_comments(book_id: i64, state: State<AppState>) -> Result<Option<String>, String> {
    with_conn(&state, |conn| library::get_comments(conn, book_id).map_err(|e| e.to_string()))
}

#[tauri::command]
fn get_books_filtered(filter: library::BookFilter, state: State<AppState>) -> Result<Vec<library::Book>, String> {
    let path = get_path(&state)?;
    with_conn(&state, |conn| library::list_books(conn, &path, &filter).map_err(|e| e.to_string()))
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            library_path: Mutex::new(None),
            conn: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            set_library_path,
            get_authors,
            get_series,
            get_tags,
            get_books_filtered,
            get_multi_text_columns,
            get_comments,
            save_last_library_path,
            load_last_library_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
