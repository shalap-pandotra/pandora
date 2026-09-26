// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod library;

use std::sync::Mutex;
use tauri::State;
use tauri::Manager;

struct AppState {
    library_path: Mutex<Option<String>>,
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
    *state.library_path.lock().unwrap() = Some(path);
    Ok(())
}

#[tauri::command]
fn get_books(state: State<AppState>) -> Result<Vec<library::Book>, String> {
    let path = state.library_path.lock().unwrap();
    let path = path.as_ref().ok_or("No library path set")?;
    let conn = library::open_library(path).map_err(|e| e.to_string())?;
    library::list_books(&conn, &library::BookFilter::default()).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_cover_path(book_path: String, state: State<AppState>) -> Option<String> {
    let path = state.library_path.lock().unwrap();
    let library_path = path.as_ref()?;
    let cover = std::path::PathBuf::from(library_path)
        .join(&book_path)
        .join("cover.jpg");
    if cover.exists() {
        Some(cover.to_string_lossy().to_string())
    } else {
        None
    }
}

#[tauri::command]
fn get_authors(state: State<AppState>) -> Result<Vec<library::FilterRow>, String> {
    let path = state.library_path.lock().unwrap();
    let path = path.as_ref().ok_or("No library path set")?;
    let conn = library::open_library(path).map_err(|e| e.to_string())?;
    library::list_authors(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_series(state: State<AppState>) -> Result<Vec<library::FilterRow>, String> {
    let path = state.library_path.lock().unwrap();
    let path = path.as_ref().ok_or("No library path set")?;
    let conn = library::open_library(path).map_err(|e| e.to_string())?;
    library::list_series(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_tags(state: State<AppState>) -> Result<Vec<library::FilterRow>, String> {
    let path = state.library_path.lock().unwrap();
    let path = path.as_ref().ok_or("No library path set")?;
    let conn = library::open_library(path).map_err(|e| e.to_string())?;
    library::list_tags(&conn).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_books_filtered(filter: library::BookFilter, state: State<AppState>) -> Result<Vec<library::Book>, String> {
    let path = state.library_path.lock().unwrap();
    let path = path.as_ref().ok_or("No library path set")?;
    let conn = library::open_library(path).map_err(|e| e.to_string())?;
    library::list_books(&conn, &filter).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_multi_text_columns(state: State<AppState>) -> Result<Vec<library::MultiTextColumn>, String> {
    let path = state.library_path.lock().unwrap();
    let path = path.as_ref().ok_or("No library path set")?;
    let conn = library::open_library(path).map_err(|e| e.to_string())?;
    library::list_multi_text_columns(&conn).map_err(|e| e.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            library_path: Mutex::new(None),
        })
.invoke_handler(tauri::generate_handler![
    set_library_path,
    get_books,          // keep or replace with get_books_filtered — your call
    get_cover_path,
    get_authors,
    get_series,
    get_tags,
    get_books_filtered,
    save_last_library_path,
    load_last_library_path,
    get_multi_text_columns
])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
