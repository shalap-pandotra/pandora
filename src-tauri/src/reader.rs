use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use tauri::http::{header, Request, Response, StatusCode};
use tauri::{AppHandle, Emitter, Manager, UriSchemeContext, UriSchemeResponder, WebviewUrl, WebviewWindowBuilder, Wry};
use zip::ZipArchive;

type Archive = Arc<Mutex<ZipArchive<File>>>;

#[derive(Default)]
pub struct ReaderState {
    next_id: AtomicU64,
    // The file each window shows, kept for as long as the window lives. If its page is
    // loaded again (a reload), it opens the same book again rather than finding nothing.
    files: Mutex<HashMap<String, PathBuf>>,
    // The calibre uuid of the book each window shows, when it came from a library.
    uuids: Mutex<HashMap<String, String>>,
    open: Mutex<HashMap<String, Archive>>,
    // Read from disk the first time they are needed.
    settings: Mutex<Option<Settings>>,
    positions: Mutex<Option<HashMap<String, Position>>>,
}

#[derive(Serialize)]
pub struct Entry {
    name: String,
    size: u64,
}

#[derive(Serialize)]
pub struct OpenedBook {
    path: String,
    file_name: String,
    entries: Vec<Entry>,
    book_uuid: Option<String>,
}

pub const PROTOCOL: &str = "book";

/// Opens a reader window for the file at `path`. The window loads the file itself once it is ready.
pub fn open_window(app: &AppHandle, path: PathBuf, uuid: Option<String>) -> Result<(), String> {
    let state = app.state::<ReaderState>();
    let label = format!("reader-{}", state.next_id.fetch_add(1, Ordering::Relaxed) + 1);
    if let Some(uuid) = uuid {
        state.uuids.lock().unwrap().insert(label.clone(), uuid);
    }
    state.files.lock().unwrap_or_else(PoisonError::into_inner).insert(label.clone(), path);
    let built = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("reader".into()))
        .title("Pandora Reader")
        .decorations(false)
        .inner_size(720.0, 900.0)
        .min_inner_size(400.0, 400.0)
        .build();
    if let Err(e) = built {
        state.uuids.lock().unwrap().remove(&label);
        state.files.lock().unwrap_or_else(PoisonError::into_inner).remove(&label);
        return Err(e.to_string());
    }
    Ok(())
}

/// Opens the file this window was created for and lists its entries.
pub fn load(app: &AppHandle, label: &str) -> Result<OpenedBook, String> {
    let state = app.state::<ReaderState>();
    let path = state.files.lock().unwrap_or_else(PoisonError::into_inner).get(label).cloned().ok_or("No book for this window")?;
    let file = File::open(&path).map_err(|e| e.to_string())?;
    let mut archive = ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut entries = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        let Ok(f) = archive.by_index_raw(i) else { continue };
        if !f.is_file() {
            continue;
        }
        if let Ok(name) = f.name() {
            entries.push(Entry { name: name.into_owned(), size: f.size() });
        }
    }
    let file_name = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    state.open.lock().unwrap_or_else(PoisonError::into_inner).insert(label.to_string(), Arc::new(Mutex::new(archive)));
    let book_uuid = state.uuids.lock().unwrap().get(label).cloned();
    Ok(OpenedBook { path: path.to_string_lossy().to_string(), file_name, entries, book_uuid })
}

/// Gives a window that already has a book open another file, once it has been checked
/// to open. The window then loads it the same way as its first one. A file that does not
/// open is refused here, so the book being read stays open.
pub fn replace(app: &AppHandle, label: &str, path: PathBuf, uuid: Option<String>) -> Result<(), String> {
    if !path.is_file() {
        return Err("That file is missing".to_string());
    }
    let file = File::open(&path).map_err(|e| e.to_string())?;
    ZipArchive::new(file).map_err(|_| "That file isn't a readable EPUB".to_string())?;
    {
        let state = app.state::<ReaderState>();
        let mut uuids = state.uuids.lock().unwrap();
        if let Some(uuid) = uuid {
            uuids.insert(label.to_string(), uuid);
        } else {
            uuids.remove(label);
        }
    }
    app.state::<ReaderState>().files.lock().unwrap_or_else(PoisonError::into_inner).insert(label.to_string(), path);
    Ok(())
}

/// Lets go of everything held for a window once it is gone.
pub fn forget(app: &AppHandle, label: &str) {
    app.state::<ReaderState>().uuids.lock().unwrap().remove(label);
    let state = app.state::<ReaderState>();
    state.files.lock().unwrap_or_else(PoisonError::into_inner).remove(label);
    state.open.lock().unwrap_or_else(PoisonError::into_inner).remove(label);
}

/// Serves `book://localhost/<window label>/<entry name>`, with the whole path percent-encoded.
pub fn protocol(ctx: UriSchemeContext<'_, Wry>, request: Request<Vec<u8>>, responder: UriSchemeResponder) {
    let app = ctx.app_handle().clone();
    let raw = request.uri().path().trim_start_matches('/').to_string();
    std::thread::spawn(move || {
        let decoded = percent_encoding::percent_decode_str(&raw).decode_utf8_lossy().to_string();
        // However the lookup goes, the page gets an answer. A panic here would otherwise
        // leave its request waiting forever.
        let response = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match decoded.split_once('/') {
            Some((label, name)) => read_entry(&app, label, name),
            None => Err(StatusCode::BAD_REQUEST),
        }))
        .unwrap_or(Err(StatusCode::INTERNAL_SERVER_ERROR));
        responder.respond(response.unwrap_or_else(|status| {
            Response::builder()
                .status(status)
                .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Vec::new())
                .unwrap()
        }));
    });
}

fn read_entry(app: &AppHandle, label: &str, name: &str) -> Result<Response<Vec<u8>>, StatusCode> {
    let archive = app
        .state::<ReaderState>()
        .open
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get(label)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;
    let mut archive = archive.lock().unwrap_or_else(PoisonError::into_inner);
    let mut entry = archive.by_name(name).map_err(|_| StatusCode::NOT_FOUND)?;
    // The size in a zip header is only a claim. It is never trusted for the allocation, and
    // reading stops at a sane limit however far the entry inflates.
    const MAX_ENTRY: u64 = 256 * 1024 * 1024;
    let mut data = Vec::with_capacity(entry.size().min(16 * 1024 * 1024) as usize);
    entry
        .by_ref()
        .take(MAX_ENTRY + 1)
        .read_to_end(&mut data)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if data.len() as u64 > MAX_ENTRY {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(data)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

// Settings shared by every reader window, saved in one small file.

const FONT_STEP_MIN: i32 = -5;
const FONT_STEP_MAX: i32 = 10;
const PADDING_MAX: u32 = 400;

#[derive(Serialize, Deserialize, Clone, PartialEq)]
#[serde(default)]
pub struct Padding {
    top: u32,
    right: u32,
    bottom: u32,
    left: u32,
}

impl Default for Padding {
    fn default() -> Self {
        Padding { top: 32, right: 48, bottom: 32, left: 48 }
    }
}

#[derive(Serialize, Deserialize, Clone, Default, PartialEq)]
#[serde(default)]
pub struct Settings {
    font_step: i32,
    padding: Padding,
    /// Two pages side by side, turned a spread at a time, instead of one scrolling column.
    two_columns: bool,
}

impl Settings {
    fn clamped(mut self) -> Self {
        self.font_step = self.font_step.clamp(FONT_STEP_MIN, FONT_STEP_MAX);
        for side in [&mut self.padding.top, &mut self.padding.right, &mut self.padding.bottom, &mut self.padding.left] {
            *side = (*side).min(PADDING_MAX);
        }
        self
    }
}

#[derive(Serialize, Clone)]
struct SettingsChanged {
    settings: Settings,
    from: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Position {
    cfi: String,
    fraction: f64,
}

fn data_file(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(name))
}

fn read_json<T: serde::de::DeserializeOwned + Default>(app: &AppHandle, name: &str) -> T {
    let Ok(path) = data_file(app, name) else { return T::default() };
    let parsed = match std::fs::read_to_string(&path) {
        // Nothing saved yet is normal.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return T::default(),
        Err(e) => Err(e.to_string()),
        Ok(text) => serde_json::from_str::<T>(&text).map_err(|e| e.to_string()),
    };
    parsed.unwrap_or_else(|why| {
        // The file is there but unusable. A copy is kept before the next save replaces it,
        // so one bad moment does not cost every saved position.
        eprintln!("{name} could not be read ({why}); keeping a copy as {name}.bak");
        let _ = std::fs::copy(&path, path.with_extension("json.bak"));
        T::default()
    })
}

// Written to a temporary file first, so a crash mid-write never leaves a broken file.
fn write_json<T: Serialize>(app: &AppHandle, name: &str, value: &T) -> Result<(), String> {
    let path = data_file(app, name)?;
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

const SETTINGS_FILE: &str = "reader-settings.json";
const POSITIONS_FILE: &str = "reader-positions.json";

pub fn settings(app: &AppHandle) -> Settings {
    let state = app.state::<ReaderState>();
    let mut cached = state.settings.lock().unwrap_or_else(PoisonError::into_inner);
    cached.get_or_insert_with(|| read_json::<Settings>(app, SETTINGS_FILE).clamped()).clone()
}

/// Saves the settings and tells every reader window about them. Returns them as stored.
pub fn set_settings(app: &AppHandle, from: &str, settings: Settings) -> Result<Settings, String> {
    let settings = settings.clamped();
    let state = app.state::<ReaderState>();
    {
        let mut cached = state.settings.lock().unwrap_or_else(PoisonError::into_inner);
        if cached.as_ref() == Some(&settings) {
            return Ok(settings);
        }
        write_json(app, SETTINGS_FILE, &settings)?;
        *cached = Some(settings.clone());
    }
    let _ = app.emit("reader-settings", SettingsChanged { settings: settings.clone(), from: from.to_string() });
    Ok(settings)
}

pub fn position(app: &AppHandle, key: &str) -> Option<Position> {
    let state = app.state::<ReaderState>();
    let mut cached = state.positions.lock().unwrap_or_else(PoisonError::into_inner);
    cached.get_or_insert_with(|| read_json(app, POSITIONS_FILE)).get(key).cloned()
}

pub fn set_position(app: &AppHandle, key: String, position: Position) -> Result<(), String> {
    let state = app.state::<ReaderState>();
    let mut cached = state.positions.lock().unwrap_or_else(PoisonError::into_inner);
    let all = cached.get_or_insert_with(|| read_json(app, POSITIONS_FILE));
    all.insert(key, position);
    write_json(app, POSITIONS_FILE, all)
}
