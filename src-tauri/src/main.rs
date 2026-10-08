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
    backed_up: Mutex<bool>,
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
    *state.backed_up.lock().unwrap() = false;
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

fn backup_once(app: &tauri::AppHandle, state: &State<AppState>) -> Result<(), String> {
    let mut done = state.backed_up.lock().unwrap();
    if *done {
        return Ok(());
    }
    let library = get_path(state)?;
    let src = std::path::Path::new(&library).join("metadata.db");
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| e.to_string())?
        .join("backups");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let name = std::path::Path::new(&library)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "library".into());
    let prefix = format!("{}-", name);

    // Existing backups of this library, newest first.
    let mut existing: Vec<(u64, std::path::PathBuf)> = std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            let file = path.file_name()?.to_string_lossy().to_string();
            let secs = file.strip_prefix(&prefix)?.strip_suffix(".db")?.parse::<u64>().ok()?;
            Some((secs, path))
        })
        .collect();
    existing.sort_by(|a, b| b.0.cmp(&a.0));

    // Nothing changed since the newest backup, so we already hold this exact state.
    if let Some((_, newest)) = existing.first() {
        if files_identical(&src, newest).unwrap_or(false) {
            *done = true;
            return Ok(());
        }
    }

    let dest = dir.join(format!("{}{}.db", prefix, stamp));
    std::fs::copy(&src, &dest).map_err(|e| e.to_string())?;
    *done = true;
    eprintln!("backup written to {}", dest.display());

    // Keep only the newest few backups of this library.
    for (_, old) in existing.iter().skip(MAX_BACKUPS - 1) {
        let _ = std::fs::remove_file(old);
    }
    Ok(())
}

const MAX_BACKUPS: usize = 10;

fn files_identical(a: &std::path::Path, b: &std::path::Path) -> std::io::Result<bool> {
    use std::io::Read;
    if std::fs::metadata(a)?.len() != std::fs::metadata(b)?.len() {
        return Ok(false);
    }
    let mut fa = std::io::BufReader::new(std::fs::File::open(a)?);
    let mut fb = std::io::BufReader::new(std::fs::File::open(b)?);
    let (mut ba, mut bb) = ([0u8; 65536], [0u8; 65536]);
    loop {
        let na = fa.read(&mut ba)?;
        if na == 0 {
            return Ok(true);
        }
        fb.read_exact(&mut bb[..na])?;
        if ba[..na] != bb[..na] {
            return Ok(false);
        }
    }
}

#[tauri::command]
fn get_next_series_index(series: String, state: State<AppState>) -> Result<f64, String> {
    with_conn(&state, |conn| library::next_series_index(conn, &series).map_err(|e| e.to_string()))
}

fn log_renames(app: &tauri::AppHandle, library: &str, moves: &[(String, String)]) {
    if moves.is_empty() {
        return;
    }
    let Ok(base) = app.path().app_config_dir() else { return };
    let dir = base.join("backups");
    let _ = std::fs::create_dir_all(&dir);
    let name = std::path::Path::new(library)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "library".into());
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut text = String::new();
    for (from, to) in moves {
        text.push_str(&format!("{}\t{}\t{}\n", stamp, from, to));
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(format!("{}-renames.log", name)))
    {
        let _ = f.write_all(text.as_bytes());
    }
}

#[tauri::command]
fn get_custom_editor(book_id: i64, state: State<AppState>) -> Result<Vec<library::CustomEditorColumn>, String> {
    with_conn(&state, |conn| library::custom_editor_data(conn, book_id).map_err(|e| e.to_string()))
}

#[tauri::command]
fn get_edit_extras(book_id: i64, state: State<AppState>) -> Result<library::EditExtras, String> {
    with_conn(&state, |conn| library::edit_extras(conn, book_id).map_err(|e| e.to_string()))
}

#[tauri::command(async)]
fn preview_cover(path: String) -> Result<library::CoverPreview, String> {
    library::preview_cover(&path)
}

fn check_edit(edit: &library::MetadataEdit) -> Result<(), String> {
    if let Some(stars) = edit.rating {
        if !(0..=5).contains(&stars) {
            return Err("Rating must be between 0 and 5 stars".to_string());
        }
    }
    if let Some(i) = edit.series_index {
        if !i.is_finite() {
            return Err("Series number must be a number".to_string());
        }
    }
    for d in [&edit.date_added, &edit.published].into_iter().flatten() {
        if !library::valid_stored_date(d) {
            return Err("That date isn't valid".to_string());
        }
    }
    if let Some(codes) = &edit.languages {
        for c in codes {
            if !library::valid_language_code(c) {
                return Err(format!("\"{}\" isn't a known language code", c));
            }
        }
    }
    Ok(())
}

#[tauri::command]
async fn save_metadata_batch(
    app: tauri::AppHandle,
    items: Vec<library::BatchItem>,
    state: State<'_, AppState>,
) -> Result<(), library::BatchError> {
    let plain = |message: String| library::BatchError { book_id: None, message };
    for it in &items {
        check_edit(&it.edit)
            .map_err(|message| library::BatchError { book_id: Some(it.book_id), message })?;
    }
    backup_once(&app, &state).map_err(plain)?;
    let path = get_path(&state).map_err(plain)?;
    let app_for_log = app.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<(), library::BatchError> {
        let mut conn = library::open_library_rw(&path)
            .map_err(|e| library::BatchError { book_id: None, message: e.to_string() })?;
        let moves = library::write_metadata_batch(&mut conn, &path, &items)?;
        log_renames(&app_for_log, &path, &moves);
        Ok(())
    })
    .await
    .map_err(|e| library::BatchError { book_id: None, message: e.to_string() })?
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            library_path: Mutex::new(None),
            conn: Mutex::new(None),
            backed_up: Mutex::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            set_library_path,
            get_authors,
            get_series,
            get_tags,
            get_books_filtered,
            get_multi_text_columns,
            get_comments,
            get_next_series_index,
            save_metadata_batch,
            get_custom_editor,
            get_edit_extras,
            preview_cover,
            save_last_library_path,
            load_last_library_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
