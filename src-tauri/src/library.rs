use rusqlite::OptionalExtension;
use rusqlite::Connection;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
pub struct Book {
    pub id: i64,
    pub title: String,
    pub authors: String,
    pub series: Option<String>,
    pub series_index: Option<f64>,
    pub tags: Vec<String>,
    pub path: String,
    pub has_cover: bool,
    pub cover_path: Option<String>,
    pub cover_version: i64,
    pub rating: Option<i64>,
}

#[derive(Serialize)]
pub struct FilterRow {
    pub id: i64,
    pub name: String,
    pub count: i64,
    pub sort: String,
}

#[derive(Default, serde::Deserialize)]
pub struct BookFilter {
    pub author_id: Option<i64>,
    pub series_id: Option<i64>,
    pub tag_id: Option<i64>,
    pub search: Option<String>,
    pub custom_column_id: Option<i64>,
    pub custom_value_id: Option<i64>,
}

#[derive(Serialize)]
pub struct MultiTextColumn {
    pub id: i64,
    pub label: String,
    pub name: String,
    pub is_hierarchical: bool,
    pub values: Vec<GenreRow>, // reusing the same shape, id/value/count
}

#[derive(Serialize)]
pub struct GenreRow {
    pub id: i64,
    pub value: String,
    pub count: i64,
}

fn hierarchical_column_labels(conn: &Connection) -> Vec<String> {
    let val: Option<String> = conn
        .query_row(
            "SELECT val FROM preferences WHERE key = 'categories_using_hierarchy'",
            [],
            |row| row.get(0),
        )
        .optional()
        .unwrap_or(None);
    val.and_then(|v| serde_json::from_str::<Vec<String>>(&v).ok())
        .unwrap_or_default()
}

pub fn list_multi_text_columns(conn: &Connection) -> rusqlite::Result<Vec<MultiTextColumn>> {
    let mut col_stmt = conn.prepare(
        "SELECT id, label, name FROM custom_columns
WHERE datatype = 'text' AND is_multiple = 1 AND mark_for_delete = 0"
    )?;

    let columns: Vec<(i64, String, String)> = col_stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<_, _>>()?;

    let hierarchical = hierarchical_column_labels(conn);
    let mut result = Vec::new();

    for (col_id, label, name) in columns {
        let sql = format!(
        "SELECT cc.id, cc.value, COUNT(link.book) as cnt
FROM custom_column_{col_id} cc
JOIN books_custom_column_{col_id}_link link ON link.value = cc.id
GROUP BY cc.id
ORDER BY cc.value ASC"
    );

        let mut val_stmt = conn.prepare(&sql)?;
        let values = val_stmt
            .query_map([], |row| {
                Ok(GenreRow { id: row.get(0)?, value: row.get(1)?, count: row.get(2)? })
            })?
            .collect::<Result<_, _>>()?;

        let is_hierarchical = hierarchical.contains(&format!("#{}", label));
        result.push(MultiTextColumn { id: col_id, label, name, is_hierarchical, values });
    }

    Ok(result)
}

pub fn list_authors(conn: &Connection) -> rusqlite::Result<Vec<FilterRow>> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.name, COUNT(bal.book) as cnt, COALESCE(a.sort, a.name)
FROM authors a
JOIN books_authors_link bal ON bal.author = a.id
GROUP BY a.id
ORDER BY cnt DESC, a.name ASC"
    )?;

    let rows = stmt.query_map([], |row| {
        let name: String = row.get(1)?;
        Ok(FilterRow { id: row.get(0)?, name: restore_author_name(&name), count: row.get(2)?, sort: row.get(3)? })
    })?;
    rows.collect()
}
pub fn list_series(conn: &Connection) -> rusqlite::Result<Vec<FilterRow>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.name, COUNT(bsl.book) as cnt, COALESCE(s.sort, s.name)
FROM series s
JOIN books_series_link bsl ON bsl.series = s.id
GROUP BY s.id
ORDER BY cnt DESC, s.name ASC"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(FilterRow { id: row.get(0)?, name: row.get(1)?, count: row.get(2)?, sort: row.get(3)? })
    })?;
    rows.collect()
}

pub fn list_tags(conn: &Connection) -> rusqlite::Result<Vec<FilterRow>> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.name, COUNT(btl.book) as cnt
FROM tags t
JOIN books_tags_link btl ON btl.tag = t.id
GROUP BY t.id
ORDER BY cnt DESC, t.name ASC"
    )?;
    let rows = stmt.query_map([], |row| {
        let name: String = row.get(1)?;
        Ok(FilterRow { id: row.get(0)?, sort: name.clone(), name, count: row.get(2)? })
    })?;
    rows.collect()
}

fn restore_author_name(name: &str) -> String {
    name.replace('|', ",")
}

pub fn title_sort(title: &str) -> String {
    let t = title.trim();
    if let Some(idx) = t.find(char::is_whitespace) {
        let first = &t[..idx];
        let rest = t[idx..].trim_start();
        let lower = first.to_lowercase();
        if !rest.is_empty() && (lower == "a" || lower == "an" || lower == "the") {
            return format!("{}, {}", rest, first);
        }
    }
    t.to_string()
}

const BAD_FILENAME_CHARS: [char; 10] = ['\\', '|', '?', '*', '<', '"', ':', '>', '+', '/'];
const WINDOWS_RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

const AUTHOR_PREFIXES: [&str; 5] = ["mr", "mrs", "ms", "dr", "prof"];
const AUTHOR_COPY_WORDS: [&str; 12] = [
    "agency", "corporation", "company", "co.", "council", "committee", "inc.", "institute",
    "national", "society", "club", "team",
];
const AUTHOR_SUFFIXES: [&str; 13] = [
    "jr", "sr", "inc", "ph.d", "phd", "md", "m.d", "i", "ii", "iii", "iv", "junior", "senior",
];

// Like Python's os.path.splitext: a name made only of dots has no extension.
fn split_ext(s: &str) -> (&str, &str) {
    match s.rfind('.') {
        None => (s, ""),
        Some(i) => {
            if s[..i].chars().all(|c| c == '.') {
                (s, "")
            } else {
                (&s[..i], &s[i..])
            }
        }
    }
}

fn sanitize_file_name(name: &str) -> String {
    let replaced: String = name
        .chars()
        .map(|c| if BAD_FILENAME_CHARS.contains(&c) || (c as u32) < 32 { '_' } else { c })
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect();
    let trimmed = replaced.trim();
    let (base, ext) = split_ext(trimmed);
    let base = if !base.is_empty() && base.chars().all(|c| c == '.') {
        "_".to_string()
    } else {
        base.to_string()
    };
    let mut one = format!("{}{}", base.replace("..", "_"), ext);
    if one.ends_with('.') || one.ends_with(' ') {
        one.pop();
        one.push('_');
    }
    if one.starts_with('.') {
        one.replace_range(..1, "_");
    }
    one
}

fn ascii_filename(orig: &str) -> String {
    let ascii = deunicode::deunicode(orig).replace('?', "_");
    sanitize_file_name(&ascii)
}

fn path_limit(library_path: &str) -> usize {
    #[cfg(windows)]
    {
        let mut limit = 40usize;
        let len = library_path.chars().count();
        if len + 4 * limit + 10 > 259 {
            limit = 259usize.saturating_sub(len + 10) / 4;
        }
        limit
    }
    #[cfg(not(windows))]
    {
        let _ = library_path;
        100
    }
}

// Folder for a book, relative to the library. Uses only the first author.
pub fn book_dir(library_path: &str, book_id: i64, title: &str, first_author: &str) -> String {
    let id_suffix = format!(" ({})", book_id);
    let l = path_limit(library_path).saturating_sub(id_suffix.len() / 2 + 2).max(1);
    let mut author: String = ascii_filename(first_author).chars().take(l).collect();
    let mut title_part: String = ascii_filename(title).chars().take(l).collect();
    while author.ends_with(' ') || author.ends_with('.') {
        author.pop();
    }
    if author.is_empty() {
        author = "Unknown".to_string();
    }
    if WINDOWS_RESERVED.contains(&author.to_uppercase().as_str()) {
        author.push('w');
    }
    if title_part.is_empty() {
        title_part = "Unknown".to_string();
    }
    format!("{}/{}{}", author, title_part, id_suffix)
}

// Base name shared by every format file of a book, without the extension.
pub fn book_file_base(library_path: &str, title: &str, first_author: &str) -> String {
    // Room is reserved for the extension, at least 14 characters, so half of that is 7.
    let l = path_limit(library_path).saturating_sub(7 + 2).max(1);
    let author: String = ascii_filename(first_author).chars().take(l).collect();
    let title: String = ascii_filename(title).chars().take(l).collect();
    let mut name = format!("{} - {}", title, author);
    while name.ends_with('.') {
        name.pop();
    }
    name
}

pub fn author_sort_for(stored_name: &str) -> String {
    let author = stored_name.replace('|', ",");
    let author = author.trim();
    let mut tokens: Vec<&str> = author.split_whitespace().collect();
    if tokens.len() < 2 {
        return author.to_string();
    }
    // Names containing a "copy word" like Corporation are left unchanged.
    if tokens.iter().any(|t| AUTHOR_COPY_WORDS.contains(&t.to_lowercase().as_str())) {
        return author.to_string();
    }
    // Leading prefixes such as Dr. are dropped from the sort entirely.
    let is_prefix =
    |t: &str| AUTHOR_PREFIXES.contains(&t.trim_end_matches('.').to_lowercase().as_str());
    while !tokens.is_empty() && is_prefix(tokens[0]) {
        tokens.remove(0);
    }
    if tokens.is_empty() {
        return author.to_string();
    }
    // Trailing suffixes such as Jr. move to the end of the result.
    let is_suffix =
    |t: &str| AUTHOR_SUFFIXES.contains(&t.trim_end_matches('.').to_lowercase().as_str());
    let mut suffix: Vec<&str> = Vec::new();
    while !tokens.is_empty() && is_suffix(tokens[tokens.len() - 1]) {
        suffix.insert(0, tokens.pop().unwrap());
    }
    if tokens.is_empty() {
        return author.to_string();
    }
    // A name that already contains a comma is copied unchanged.
    if tokens.join("").contains(',') {
        return author.to_string();
    }
    let count = tokens.len();
    let surname = tokens.pop().unwrap();
    let mut out = surname.to_string();
    if count > 1 {
        out.push(',');
        out.push(' ');
        out.push_str(&tokens.join(" "));
    }
    if !suffix.is_empty() {
        out.push(' ');
        out.push_str(&suffix.join(" "));
    }
    out
}

#[cfg(test)]
mod author_sort_tests {
    use super::author_sort_for;

    // Every expected value below was copied from what calibre produced on your library.
    #[test]
    fn author_sort_matches_calibre() {
        let cases = [
            ("Isaac Asimov", "Asimov, Isaac"),
            ("Kim Stanley Robinson", "Robinson, Kim Stanley"),
            ("Baoshu", "Baoshu"),
            ("Nemo Ramjet| Isaiah Nichols", "Nemo Ramjet, Isaiah Nichols"),
            ("Gabriel García Márquez", "Márquez, Gabriel García"),
            ("रामधारी सि\u{902}ह दिनकर", "दिनकर, रामधारी सि\u{902}ह"),
            ("Dr. Seuss", "Seuss"),
            ("Acme Corporation", "Acme Corporation"),
            ("Sammy Davis Sr.", "Davis, Sammy Sr."),
            ("John Smith Ph.D.", "Smith, John Ph.D."),
        ];
        for (name, expected) in cases {
            assert_eq!(author_sort_for(name), expected, "name: {}", name);
        }
    }
}

fn resolve_cover(library_path: &str, book_path: &str, has_cover: bool) -> Option<String> {

    if !has_cover { return None; }
    let cover = std::path::Path::new(library_path).join(book_path).join("cover.jpg");
    cover.exists().then(|| cover.to_string_lossy().to_string())
}

#[derive(Serialize)]
pub struct PickerBook {
    pub id: i64,
    pub title: String,
    pub authors: String,
}

/// Every book that has an EPUB, by title and author only, for the reader's Ctrl+O picker.
pub fn epub_books(conn: &Connection) -> rusqlite::Result<Vec<PickerBook>> {
    let mut stmt = conn.prepare(
        "SELECT b.id, b.title,
            (SELECT group_concat(replace(a.name, '|', ','), ' & ')
             FROM authors a JOIN books_authors_link bal ON bal.author = a.id
             WHERE bal.book = b.id) AS authors
         FROM books b
         WHERE EXISTS (SELECT 1 FROM data d WHERE d.book = b.id AND upper(d.format) = 'EPUB')
         ORDER BY b.sort",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(PickerBook {
            id: row.get(0)?,
            title: row.get(1)?,
            authors: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
        })
    })?;
    rows.collect()
}

/// Where a book's EPUB sits on disk, if it has one.
pub fn epub_path(conn: &Connection, library_path: &str, book_id: i64) -> rusqlite::Result<Option<PathBuf>> {
    conn.query_row(
        "SELECT b.path, d.name FROM books b JOIN data d ON d.book = b.id
         WHERE b.id = ?1 AND upper(d.format) = 'EPUB'",
        [book_id],
        |row| {
            let folder: String = row.get(0)?;
            let name: String = row.get(1)?;
            Ok(PathBuf::from(library_path).join(folder).join(format!("{}.epub", name)))
        },
    )
    .optional()
}

/// calibre's own uuid for a book, if it has one.
pub fn book_uuid(conn: &Connection, book_id: i64) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT uuid FROM books WHERE id = ?1", [book_id], |row| row.get::<_, Option<String>>(0))
        .optional()
        .map(|found| found.flatten().filter(|u| !u.trim().is_empty()))
}

pub fn book_title_by_uuid(conn: &Connection, uuid: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row("SELECT title FROM books WHERE uuid = ?1", [uuid], |row| row.get::<_, String>(0)).optional()
}

pub fn book_uuid_and_title(conn: &Connection, book_id: i64) -> rusqlite::Result<Option<(String, String)>> {
    conn.query_row("SELECT uuid, title FROM books WHERE id = ?1", [book_id], |row| {
        Ok((row.get::<_, Option<String>>(0)?, row.get::<_, String>(1)?))
    })
    .optional()
    .map(|found| found.and_then(|(uuid, title)| uuid.filter(|u| !u.trim().is_empty()).map(|u| (u, title))))
}

pub fn open_library(library_path: &str) -> rusqlite::Result<Connection> {
    let db_path = PathBuf::from(library_path).join("metadata.db");
    // Read-only for everything except saving edits
    Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
}

pub fn open_library_rw(library_path: &str) -> rusqlite::Result<Connection> {
    let db_path = PathBuf::from(library_path).join("metadata.db");
    let conn = Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    conn.busy_timeout(std::time::Duration::from_millis(2000))?;
    // Calibre's triggers call this function, and it lives outside the database.
    conn.create_scalar_function(
        "title_sort",
        1,
        rusqlite::functions::FunctionFlags::SQLITE_UTF8
        | rusqlite::functions::FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            let t: String = ctx.get(0)?;
            Ok(title_sort(&t))
        },
    )?;
    Ok(conn)
}

#[derive(serde::Deserialize)]
pub struct MetadataEdit {
    pub cover: Option<CoverEdit>, // None means unchanged
    pub tags: Option<Vec<String>>, // None means unchanged
    pub series: Option<String>,    // None means unchanged, empty string clears
    pub series_index: Option<f64>, // None means unchanged
    pub title: Option<String>,     // None means unchanged
    pub authors: Option<Vec<String>>, // None means unchanged, one entry per author
    pub custom: Option<Vec<CustomEdit>>, // None means unchanged
    pub publisher: Option<String>,       // None means unchanged, empty clears
    pub identifiers: Option<Vec<IdentifierEdit>>, // None means unchanged
    pub languages: Option<Vec<String>>, // three letter codes in order, None means unchanged
    pub date_added: Option<String>,      // stored form, None means unchanged
    pub published: Option<String>,       // stored form, None means unchanged
    pub comments: Option<String>, // None means unchanged
    pub rating: Option<i64>,      // None means unchanged, 0 clears, 1 to 5 sets stars
}

// Each field only runs if it was actually changed. One transaction covers all of
// them, so a failure part way through leaves the library untouched.
#[derive(serde::Deserialize)]
pub struct BatchItem {
    pub book_id: i64,
    pub edit: MetadataEdit,
}

#[derive(Serialize)]
pub struct BatchError {
    pub book_id: Option<i64>,
    pub message: String,
}

// Saves several books in one transaction. Edits are applied in the order given, so when two
// books touch the same shared value (a recase, say), the later edit wins, as it would if they
// were saved one at a time. If any book fails, every folder move and cover change made so far
// is undone, and nothing is committed.
pub fn write_metadata_batch(
    conn: &mut Connection,
    library_path: &str,
    items: &[BatchItem],
) -> Result<Vec<(String, String)>, BatchError> {
    let tx = conn
        .transaction()
        .map_err(|e| BatchError { book_id: None, message: e.to_string() })?;
    let mut logs: Vec<FsLog> = Vec::new();
    for item in items {
        match apply_edit(&tx, library_path, item.book_id, &item.edit) {
            Ok(log) => logs.push(log),
            Err(message) => {
                for log in logs.iter().rev() {
                    log.undo();
                }
                return Err(BatchError { book_id: Some(item.book_id), message });
            }
        }
    }
    if let Err(e) = tx.commit() {
        for log in logs.iter().rev() {
            log.undo();
        }
        return Err(BatchError { book_id: None, message: e.to_string() });
    }
    let lib = std::path::Path::new(library_path);
    let rel = |p: &std::path::Path| -> String {
        p.strip_prefix(lib).unwrap_or(p).to_string_lossy().replace('\\', "/")
    };
    let mut moves = Vec::new();
    for log in &logs {
        for (a, b) in &log.moves {
            moves.push((rel(a.as_path()), rel(b.as_path())));
        }
    }
    Ok(moves)
}

fn db<T>(r: rusqlite::Result<T>) -> Result<T, String> {
    r.map_err(|e| e.to_string())
}

#[derive(Default)]
struct FsLog {
    moves: Vec<(PathBuf, PathBuf)>,
    created: Vec<PathBuf>,
    removed: Vec<PathBuf>,
    cover: Option<(PathBuf, Option<Vec<u8>>)>,
}

impl FsLog {
    // Best effort, in reverse order: recreate removed folders, move things back, drop new folders.
    fn undo(&self) {
        // The cover lives in the book's final folder, so it goes back before any folder moves.
        if let Some((file, previous)) = &self.cover {
            match previous {
                Some(bytes) => {
                    let _ = std::fs::write(file, bytes);
                }
                None => {
                    let _ = std::fs::remove_file(file);
                }
            }
        }
        for d in &self.removed {
            let _ = std::fs::create_dir_all(d);
        }
        for (from, to) in self.moves.iter().rev() {
            let _ = std::fs::rename(to, from);
        }
        for d in self.created.iter().rev() {
            let _ = std::fs::remove_dir(d);
        }
    }
}

struct BookPlan {
    id: i64,
    old_path: String,
    new_path: String,
    // data row id, format, old base name, new base name
    files: Vec<(i64, String, String, String)>,
}

fn author_dir_of(path: &str) -> &str {
    path.split_once('/').map(|(a, _)| a).unwrap_or(path)
}

fn title_dir_of(path: &str) -> &str {
    path.split_once('/').map(|(_, t)| t).unwrap_or("")
}

fn touch(tx: &Connection, book_id: i64) -> rusqlite::Result<()> {
    tx.execute(
        "UPDATE books SET last_modified = strftime('%Y-%m-%d %H:%M:%f+00:00', 'now') WHERE id = ?1",
        [book_id],
    )?;
    Ok(())
}

fn apply_title(tx: &Connection, book_id: i64, title: &str) -> rusqlite::Result<()> {
    let t = title.trim();
    let t = if t.is_empty() { "Unknown" } else { t };
    tx.execute(
        "UPDATE books SET title = ?1, sort = title_sort(?1) WHERE id = ?2",
        rusqlite::params![t, book_id],
    )?;
    Ok(())
}

// books.author_sort is the authors' own sorts joined with " & ", in link order.
fn rebuild_author_sort(tx: &Connection, book_id: i64) -> rusqlite::Result<()> {
    let sorts: Vec<String> = {
        let mut stmt = tx.prepare(
            "SELECT a.name, a.sort FROM authors a JOIN books_authors_link l ON l.author = a.id \
                WHERE l.book = ?1 ORDER BY l.id",
        )?;
        let rows = stmt.query_map([book_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })?;
        let mut out = Vec::new();
        for r in rows {
            let (name, sort) = r?;
            out.push(sort.unwrap_or_else(|| author_sort_for(&name)));
        }
        out
    };
    tx.execute(
        "UPDATE books SET author_sort = ?1 WHERE id = ?2",
        rusqlite::params![sorts.join(" & "), book_id],
    )?;
    Ok(())
}

// After an author is renamed, calibre recomputes the sort of that author's books from the
// author names, and leaves the author rows' own sort alone. Co-authors who were not renamed
// keep their stored sort. That mixed case is a guess, not verified against calibre.
fn rebuild_author_sort_after_rename(tx: &Connection, book_id: i64, renamed: &[i64]) -> rusqlite::Result<()> {
    let parts: Vec<String> = {
        let mut stmt = tx.prepare(
            "SELECT a.id, a.name, a.sort FROM authors a JOIN books_authors_link l ON l.author = a.id \
                WHERE l.book = ?1 ORDER BY l.id",
        )?;
        let rows = stmt.query_map([book_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?))
        })?;
        let mut out = Vec::new();
        for r in rows {
            let (id, name, sort) = r?;
            if renamed.contains(&id) {
                out.push(author_sort_for(&name));
            } else {
                out.push(sort.unwrap_or_else(|| author_sort_for(&name)));
            }
        }
        out
    };
    tx.execute(
        "UPDATE books SET author_sort = ?1 WHERE id = ?2",
        rusqlite::params![parts.join(" & "), book_id],
    )?;
    Ok(())
}

// Returns the ids of authors whose name was renamed (capitalization changed).
fn apply_authors(tx: &Connection, book_id: i64, names: &[String]) -> rusqlite::Result<Vec<i64>> {
    // Commas are stored as "|", and only "&" separates authors, as in calibre.
    let mut wanted: Vec<String> = Vec::new();
    for n in names {
        let n = n.trim().replace(',', "|");
        if !n.is_empty() && !wanted.iter().any(|w| w.eq_ignore_ascii_case(&n)) {
            wanted.push(n);
        }
    }
    if wanted.is_empty() {
        wanted.push("Unknown".to_string());
    }

    let old_ids: Vec<i64> = {
        let mut stmt = tx.prepare("SELECT author FROM books_authors_link WHERE book = ?1 ORDER BY id")?;
        let rows = stmt.query_map([book_id], |row| row.get::<_, i64>(0))?;
        let found = rows.collect::<Result<Vec<i64>, _>>()?;
        found
    };

    let mut new_ids: Vec<i64> = Vec::new();
    let mut renamed: Vec<i64> = Vec::new();
    for name in &wanted {
        let existing: Option<(i64, String, Option<String>)> = tx
            .query_row(
                "SELECT id, name, sort FROM authors WHERE name = ?1 COLLATE NOCASE",
                [name],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let id = match existing {
            Some((id, current, _sort)) => {
                if current != *name {
                    // Calibre renames the shared row and leaves the author's own sort alone.
                    tx.execute(
                        "UPDATE authors SET name = ?1 WHERE id = ?2",
                        rusqlite::params![name, id],
                    )?;
                    renamed.push(id);
                }
                id
            }
            None => {
                tx.execute(
                    "INSERT INTO authors (name, sort) VALUES (?1, ?2)",
                    rusqlite::params![name, author_sort_for(name)],
                )?;
                tx.last_insert_rowid()
            }
        };
        if !new_ids.contains(&id) {
            new_ids.push(id);
        }
    }

    if new_ids != old_ids {
        tx.execute("DELETE FROM books_authors_link WHERE book = ?1", [book_id])?;
        for id in &new_ids {
            tx.execute(
                "INSERT INTO books_authors_link (book, author) VALUES (?1, ?2)",
                [book_id, *id],
            )?;
        }
        // Calibre drops authors that no book uses anymore.
        for id in old_ids.iter().filter(|id| !new_ids.contains(id)) {
            tx.execute(
                "DELETE FROM authors WHERE id = ?1 AND NOT EXISTS (SELECT 1 FROM books_authors_link WHERE author = ?1)",
                [*id],
            )?;
        }
    }

    rebuild_author_sort(tx, book_id)?;
    for author_id in &renamed {
        let books: Vec<i64> = {
            let mut stmt = tx.prepare("SELECT book FROM books_authors_link WHERE author = ?1")?;
            let rows = stmt.query_map([*author_id], |row| row.get::<_, i64>(0))?;
            let found = rows.collect::<Result<Vec<i64>, _>>()?;
            found
        };
        for b in books {
            rebuild_author_sort_after_rename(tx, b, &renamed)?;
            touch(tx, b)?;
        }
    }
    Ok(renamed)
}

// What a book's folder and file names should be, compared with what is stored.
fn plan_for(tx: &Connection, library_path: &str, book_id: i64) -> rusqlite::Result<BookPlan> {
    let (title, old_path): (String, String) = tx.query_row(
        "SELECT title, path FROM books WHERE id = ?1",
        [book_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let first: Option<String> = tx
        .query_row(
            "SELECT a.name FROM authors a JOIN books_authors_link l ON l.author = a.id \
                WHERE l.book = ?1 ORDER BY l.id LIMIT 1",
            [book_id],
            |row| row.get(0),
        )
        .optional()?;
    let first = first.unwrap_or_else(|| "Unknown".to_string()).replace('|', ",");
    let new_path = book_dir(library_path, book_id, &title, &first);
    let base = book_file_base(library_path, &title, &first);
    let files: Vec<(i64, String, String, String)> = {
        let mut stmt = tx.prepare("SELECT id, format, name FROM data WHERE book = ?1")?;
        let rows = stmt.query_map([book_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, base.clone()))
        })?;
        let found = rows.collect::<Result<Vec<_>, _>>()?;
        found
    };
    Ok(BookPlan { id: book_id, old_path, new_path, files })
}

// Never overwrites. A change in capitalization only goes through a temporary name.
fn move_path(from: &std::path::Path, to: &std::path::Path, log: &mut FsLog) -> Result<(), String> {
    let case_only = from != to
    && from.to_string_lossy().to_lowercase() == to.to_string_lossy().to_lowercase();
    if case_only {
        let mut tmp = from.as_os_str().to_owned();
        tmp.push(".pandora-tmp");
        let tmp = PathBuf::from(tmp);
        std::fs::rename(from, &tmp)
            .map_err(|e| format!("Could not rename {}: {}", from.display(), e))?;
        if let Err(e) = std::fs::rename(&tmp, to) {
            let _ = std::fs::rename(&tmp, from);
            return Err(format!("Could not rename {}: {}", from.display(), e));
        }
    } else {
        if to.exists() {
            return Err(format!("{} already exists", to.display()));
        }
        std::fs::rename(from, to)
            .map_err(|e| format!("Could not move {}: {}", from.display(), e))?;
    }
    log.moves.push((from.to_path_buf(), to.to_path_buf()));
    Ok(())
}

fn fs_steps(lib: &std::path::Path, plans: &[BookPlan], log: &mut FsLog) -> Result<(), String> {
    // Where each book folder is right now, relative to the library. Starts at the old path.
    let mut current: Vec<String> = plans.iter().map(|p| p.old_path.clone()).collect();

    // 1. An author folder that only changed capitalization is renamed once, for all its books.
    let mut renamed_dirs: Vec<String> = Vec::new();
    for p in plans.iter().filter(|p| !p.old_path.is_empty()) {
        let old_a = author_dir_of(&p.old_path).to_string();
        let new_a = author_dir_of(&p.new_path).to_string();
        if old_a != new_a
        && old_a.to_lowercase() == new_a.to_lowercase()
        && !renamed_dirs.contains(&old_a)
        {
            let from = lib.join(&old_a);
            if from.exists() {
                move_path(&from, &lib.join(&new_a), log)?;
                renamed_dirs.push(old_a.clone());
                for c in current.iter_mut() {
                    if author_dir_of(c) == old_a.as_str() {
                        let rebased = format!("{}/{}", new_a, title_dir_of(c));
                        *c = rebased;
                    }
                }
            }
        }
    }

    // 2. Rename the format files inside each book folder.
    for (i, p) in plans.iter().enumerate() {
        if p.old_path.is_empty() {
            continue;
        }
        let dir = lib.join(&current[i]);
        for (_, format, old_name, new_name) in &p.files {
            if old_name == new_name {
                continue;
            }
            let ext = format.to_lowercase();
            let from = dir.join(format!("{}.{}", old_name, ext));
            let to = dir.join(format!("{}.{}", new_name, ext));
            if from.exists() {
                move_path(&from, &to, log)?;
            }
        }
    }

    // 3. Move each book folder to its new location.
    for (i, p) in plans.iter().enumerate() {
        if p.old_path.is_empty() || current[i] == p.new_path {
            continue;
        }
        let from = lib.join(&current[i]);
        if !from.exists() {
            continue;
        }
        let to = lib.join(&p.new_path);
        if let Some(parent) = to.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Could not create {}: {}", parent.display(), e))?;
                log.created.push(parent.to_path_buf());
            }
        }
        move_path(&from, &to, log)?;
        current[i] = p.new_path.clone();
    }

    // 4. Remove old author folders that are now empty, like calibre does.
    let mut old_dirs: Vec<String> = plans
        .iter()
        .filter(|p| !p.old_path.is_empty())
        .map(|p| author_dir_of(&p.old_path).to_string())
        .collect();
    old_dirs.sort();
    old_dirs.dedup();
    for d in old_dirs {
        if d.is_empty() {
            continue;
        }
        let dir = lib.join(&d);
        let empty = std::fs::read_dir(&dir).map(|mut r| r.next().is_none()).unwrap_or(false);
        if empty && std::fs::remove_dir(&dir).is_ok() {
            log.removed.push(dir);
        }
    }
    Ok(())
}

fn run_fs_plans(library_path: &str, plans: &[BookPlan]) -> Result<FsLog, String> {
    let mut log = FsLog::default();
    match fs_steps(std::path::Path::new(library_path), plans, &mut log) {
        Ok(()) => Ok(log),
        Err(e) => {
            log.undo();
            Err(e)
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum CoverEdit {
    Set { path: String },
    Remove,
}

// Sizes and quality as measured on a real calibre library: a JPEG inside the box is kept
// as it is, a bigger one is scaled to fit, and anything else becomes a JPEG at quality 90.
const COVER_MAX_W: u32 = 1650;
const COVER_MAX_H: u32 = 2200;
const COVER_QUALITY: u8 = 90;

#[derive(Clone)]
struct PreparedCover {
    preview: Vec<u8>,
    bytes: Vec<u8>,
    width: u32,
    height: u32,
    note: String,
}

// Fits inside the box without changing the shape. The second side is truncated, not rounded,
// which is what calibre does (3182x4500 becomes 1555x2200).
fn fit_inside(w: u32, h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    let (w, h, mw, mh) = (w as u64, h as u64, max_w as u64, max_h as u64);
    if w * mh > h * mw {
        (mw as u32, (h * mw / w).max(1) as u32)
    } else {
        ((w * mh / h).max(1) as u32, mh as u32)
    }
}

// Transparent areas become white, since a JPEG can't be transparent.
fn flatten_on_white(img: &image::DynamicImage) -> image::RgbImage {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut out = image::RgbImage::new(w, h);
    for (x, y, p) in rgba.enumerate_pixels() {
        let a = p[3] as u32;
        let mix = |c: u8| ((c as u32 * a + 255 * (255 - a)) / 255) as u8;
        out.put_pixel(x, y, image::Rgb([mix(p[0]), mix(p[1]), mix(p[2])]));
    }
    out
}

fn encode_jpeg(img: &image::DynamicImage, quality: u8) -> Result<Vec<u8>, String> {
    let rgb = flatten_on_white(img);
    let mut out: Vec<u8> = Vec::new();
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
    image::DynamicImage::ImageRgb8(rgb)
        .write_with_encoder(encoder)
        .map_err(|e| format!("Could not encode the cover: {}", e))?;
    Ok(out)
}

// The last prepared cover is kept in memory. The dialog prepares it when a file is picked,
// and saving reuses it instead of doing the work a second time.
static COVER_CACHE: std::sync::Mutex<Option<(String, u64, u128, PreparedCover)>> =
std::sync::Mutex::new(None);

fn file_key(path: &str) -> Option<(u64, u128)> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
    Some((meta.len(), modified))
}

fn make_preview(img: &image::DynamicImage) -> Result<Vec<u8>, String> {
    // thumbnail() is a fast scaler, which is plenty for an on-screen preview.
    let thumb = if img.width() > 600 || img.height() > 800 {
        img.thumbnail(600, 800)
    } else {
        img.clone()
    };
    encode_jpeg(&thumb, 85)
}

fn prepare_cover(path: &str) -> Result<PreparedCover, String> {
    let key = file_key(path);
    if let Some((len, modified)) = key {
        if let Some((p, l, m, prepared)) = COVER_CACHE.lock().unwrap().as_ref() {
            if p.as_str() == path && *l == len && *m == modified {
                return Ok(prepared.clone());
            }
        }
    }
    let prepared = prepare_cover_uncached(path)?;
    if let Some((len, modified)) = key {
        *COVER_CACHE.lock().unwrap() = Some((path.to_string(), len, modified, prepared.clone()));
    }
    Ok(prepared)
}

fn prepare_cover_uncached(path: &str) -> Result<PreparedCover, String> {
    let original = std::fs::read(path).map_err(|e| format!("Could not read {}: {}", path, e))?;
    let img = image::load_from_memory(&original)
        .map_err(|_| format!("{} is not a valid picture", path))?;
    let is_jpeg = matches!(image::guess_format(&original), Ok(image::ImageFormat::Jpeg));
    let (w, h) = (img.width(), img.height());
    let too_big = w > COVER_MAX_W || h > COVER_MAX_H;
    let preview = make_preview(&img)?;
    if is_jpeg && !too_big {
        return Ok(PreparedCover {
            bytes: original,
            preview,
            width: w,
            height: h,
            note: "Used as it is".to_string(),
        });
    }
    let (nw, nh) = if too_big { fit_inside(w, h, COVER_MAX_W, COVER_MAX_H) } else { (w, h) };
    let scaled = if too_big {
        img.resize_exact(nw, nh, image::imageops::FilterType::CatmullRom)
    } else {
        img
    };
    let bytes = encode_jpeg(&scaled, COVER_QUALITY)?;
    let note = match (is_jpeg, too_big) {
        (true, true) => format!("Scaled down from {}x{} to fit {}x{}", w, h, COVER_MAX_W, COVER_MAX_H),
        (false, true) => format!("Converted to JPEG and scaled down from {}x{}", w, h),
        _ => "Converted to JPEG".to_string(),
    };
    Ok(PreparedCover { bytes, preview, width: nw, height: nh, note })
}

#[derive(Serialize)]
pub struct CoverPreview {
    pub data_url: String,
    pub width: u32,
    pub height: u32,
    pub note: String,
}

// What the dialog shows after a file is picked. A bad file is rejected here.
pub fn preview_cover(path: &str) -> Result<CoverPreview, String> {
    let prepared = prepare_cover(path)?;
    use base64::Engine;
    Ok(CoverPreview {
        data_url: format!(
            "data:image/jpeg;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&prepared.preview)
        ),
        width: prepared.width,
        height: prepared.height,
        note: prepared.note,
    })
}

// Writes next to the target and renames over it, so a failure never leaves half a cover.
fn write_replacing(file: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    let mut tmp = file.as_os_str().to_owned();
    tmp.push(".pandora-tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes).map_err(|e| format!("Could not write the cover: {}", e))?;
    if let Err(e) = std::fs::rename(&tmp, file) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("Could not replace the cover: {}", e));
    }
    Ok(())
}

// Calibre keeps a book's cover as cover.jpg in its folder, plus a has_cover flag.
// The previous file is held in memory until the save is committed, so a failure puts it back.
fn apply_cover(tx: &Connection, library_path: &str, book_id: i64, cover: &CoverEdit, log: &mut FsLog) -> Result<(), String> {
    let rel: String = db(tx.query_row("SELECT path FROM books WHERE id = ?1", [book_id], |row| row.get(0)))?;
    let file = std::path::Path::new(library_path).join(rel).join("cover.jpg");
    log.cover = Some((file.clone(), std::fs::read(&file).ok()));
    match cover {
        CoverEdit::Remove => {
            if file.exists() {
                std::fs::remove_file(&file).map_err(|e| format!("Could not remove the cover: {}", e))?;
            }
            db(tx.execute("UPDATE books SET has_cover = 0 WHERE id = ?1", [book_id]))?;
        }
        CoverEdit::Set { path } => {
            let prepared = prepare_cover(path)?;
            write_replacing(&file, &prepared.bytes)?;
            db(tx.execute("UPDATE books SET has_cover = 1 WHERE id = ?1", [book_id]))?;
        }
    }
    Ok(())
}

fn apply_edit(tx: &Connection, library_path: &str, book_id: i64, edit: &MetadataEdit) -> Result<FsLog, String> {
    db(apply_basic_fields(tx, book_id, edit))?;

    if let Some(title) = &edit.title {
        db(apply_title(tx, book_id, title))?;
    }
    let mut renamed: Vec<i64> = Vec::new();
    if let Some(names) = &edit.authors {
        renamed = db(apply_authors(tx, book_id, names))?;
    }
    if let Some(custom) = &edit.custom {
        for c in custom {
            apply_custom_values(tx, book_id, c.column_id, &c.values)?;
        }
    }

    // Books whose folder or file names may change: this book, plus every book whose first
    // author is an author that was just renamed.
    let mut affected: Vec<i64> = Vec::new();
    if edit.title.is_some() || edit.authors.is_some() {
        affected.push(book_id);
    }
    for author_id in &renamed {
        let ids: Vec<i64> = {
            let mut stmt = db(tx.prepare(
                "SELECT l.book FROM books_authors_link l WHERE l.author = ?1 \
                    AND l.id = (SELECT MIN(id) FROM books_authors_link WHERE book = l.book)",
            ))?;
            let rows = db(stmt.query_map([*author_id], |row| row.get::<_, i64>(0)))?;
            let found = db(rows.collect::<Result<Vec<i64>, _>>())?;
            found
        };
        for id in ids {
            if !affected.contains(&id) {
                affected.push(id);
            }
        }
    }

    let mut plans: Vec<BookPlan> = Vec::new();
    for id in &affected {
        let plan = db(plan_for(tx, library_path, *id))?;
        if plan.old_path != plan.new_path || plan.files.iter().any(|f| f.2 != f.3) {
            plans.push(plan);
        }
    }

    let mut log = run_fs_plans(library_path, &plans)?;

    let record = || -> rusqlite::Result<()> {
        for p in &plans {
            if p.old_path != p.new_path {
                tx.execute(
                    "UPDATE books SET path = ?1 WHERE id = ?2",
                    rusqlite::params![p.new_path, p.id],
                )?;
            }
            for f in &p.files {
                if f.2 != f.3 {
                    tx.execute(
                        "UPDATE data SET name = ?1 WHERE id = ?2",
                        rusqlite::params![f.3, f.0],
                    )?;
                }
            }
        }
        Ok(())
    };
    if let Err(e) = record() {
        log.undo();
        return Err(e.to_string());
    }

    if let Some(cover) = &edit.cover {
        if let Err(e) = apply_cover(tx, library_path, book_id, cover, &mut log) {
            log.undo();
            return Err(e);
        }
    }
    Ok(log)
}

#[derive(Serialize)]
pub struct CustomEditorColumn {
    pub id: i64,
    pub name: String,
    pub is_names: bool,
    pub values: Vec<String>,
    pub suggestions: Vec<String>,
}

#[derive(serde::Deserialize)]
pub struct CustomEdit {
    pub column_id: i64,
    pub values: Vec<String>,
}

// Multi value text columns the user may edit. Skips columns calibre marked as not editable
// or about to be deleted. Returns (id, name, is_names).
fn editable_text_columns(conn: &Connection) -> rusqlite::Result<Vec<(i64, String, bool)>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, display FROM custom_columns \
            WHERE datatype = 'text' AND is_multiple = 1 AND mark_for_delete = 0 AND editable = 1 \
            ORDER BY id",
    )?;
    let rows = stmt.query_map([], |row| {
        let display: Option<String> = row.get(2)?;
        let is_names = display
            .and_then(|d| serde_json::from_str::<serde_json::Value>(&d).ok())
            .and_then(|v| v.get("is_names").and_then(|b| b.as_bool()))
            .unwrap_or(false);
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, is_names))
    })?;
    let found = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(found)
}

fn query_strings(conn: &Connection, sql: &str, params: impl rusqlite::Params) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params, |row| row.get::<_, String>(0))?;
    let found = rows.collect::<Result<Vec<String>, _>>()?;
    Ok(found)
}

// What the edit dialog needs: each editable column, this book's values, and every existing
// value of that column for suggestions.
pub fn custom_editor_data(conn: &Connection, book_id: i64) -> rusqlite::Result<Vec<CustomEditorColumn>> {
    let mut out = Vec::new();
    for (id, name, is_names) in editable_text_columns(conn)? {
        let mine = query_strings(
            conn,
            &format!(
            "SELECT c.value FROM custom_column_{id} c \
                JOIN books_custom_column_{id}_link l ON l.value = c.id \
                WHERE l.book = ?1 ORDER BY l.id"
        ),
            rusqlite::params![book_id],
        )?;
        let all = query_strings(
            conn,
            &format!("SELECT value FROM custom_column_{id} ORDER BY value"),
            rusqlite::params![],
        )?;
        let show = |v: String| if is_names { v.replace('|', ",") } else { v };
        out.push(CustomEditorColumn {
            id,
            name,
            is_names,
            values: mine.into_iter().map(&show).collect(),
            suggestions: all.into_iter().map(&show).collect(),
        });
    }
    Ok(out)
}

// Same rules as tags: match ignoring case, rename the shared row when the capitalization
// differs, and delete values no book uses anymore.
fn write_custom_values(conn: &Connection, book_id: i64, col: i64, is_names: bool, values: &[String]) -> rusqlite::Result<()> {
    let table = format!("custom_column_{}", col);
    let link = format!("books_custom_column_{}_link", col);

    let mut wanted: Vec<String> = Vec::new();
    for v in values {
        let v = v.trim();
        let v = if is_names { v.replace(',', "|") } else { v.to_string() };
        if !v.is_empty() && !wanted.iter().any(|w| w.eq_ignore_ascii_case(&v)) {
            wanted.push(v);
        }
    }

    let old_ids: Vec<i64> = {
        let mut stmt = conn.prepare(&format!("SELECT value FROM {} WHERE book = ?1", link))?;
        let rows = stmt.query_map([book_id], |row| row.get::<_, i64>(0))?;
        let found = rows.collect::<Result<Vec<i64>, _>>()?;
        found
    };

    let mut new_ids: Vec<i64> = Vec::new();
    for name in &wanted {
        let existing: Option<(i64, String)> = conn
            .query_row(
                &format!("SELECT id, value FROM {} WHERE value = ?1 COLLATE NOCASE", table),
                [name],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let id = match existing {
            Some((id, current)) => {
                if current != *name {
                    conn.execute(
                        &format!("UPDATE {} SET value = ?1 WHERE id = ?2", table),
                        rusqlite::params![name, id],
                    )?;
                }
                id
            }
            None => {
                conn.execute(&format!("INSERT INTO {} (value) VALUES (?1)", table), [name])?;
                conn.last_insert_rowid()
            }
        };
        if !new_ids.contains(&id) {
            new_ids.push(id);
        }
    }

    for id in old_ids.iter().filter(|id| !new_ids.contains(id)) {
        conn.execute(
            &format!("DELETE FROM {} WHERE book = ?1 AND value = ?2", link),
            [book_id, *id],
        )?;
        conn.execute(
            &format!(
                "DELETE FROM {} WHERE id = ?1 AND NOT EXISTS (SELECT 1 FROM {} WHERE value = ?1)",
                table, link
            ),
            [*id],
        )?;
    }
    for id in new_ids.iter().filter(|id| !old_ids.contains(id)) {
        conn.execute(
            &format!("INSERT INTO {} (book, value) VALUES (?1, ?2)", link),
            [book_id, *id],
        )?;
    }
    Ok(())
}

// Refuses any column that isn't an editable multi value text column.
fn apply_custom_values(tx: &Connection, book_id: i64, column_id: i64, values: &[String]) -> Result<(), String> {
    let cols = db(editable_text_columns(tx))?;
    let (id, _name, is_names) = cols
        .into_iter()
        .find(|c| c.0 == column_id)
        .ok_or_else(|| "That custom column can't be edited".to_string())?;
    db(write_custom_values(tx, book_id, id, is_names, values))
}

#[derive(serde::Deserialize)]
pub struct IdentifierEdit {
    #[serde(rename = "type")]
    pub kind: String,
    pub val: String,
}

// "YYYY-MM-DD HH:MM:SS+00:00", the form calibre stores dates in. 0101-01-01 means undefined.
pub fn valid_stored_date(s: &str) -> bool {
    if !s.is_ascii() || s.len() != 25 || !s.ends_with("+00:00") {
        return false;
    }
    let b = s.as_bytes();
    let digits = |from: usize, to: usize| b[from..to].iter().all(|c| c.is_ascii_digit());
    digits(0, 4) && b[4] == b'-' && digits(5, 7) && b[7] == b'-' && digits(8, 10)
    && b[10] == b' ' && digits(11, 13) && b[13] == b':' && digits(14, 16)
    && b[16] == b':' && digits(17, 19)
}

// Same rules as series: match ignoring case, rename the shared row when the capitalization
// differs, and delete a publisher no book uses anymore.
fn apply_publisher(conn: &Connection, book_id: i64, name: &str) -> rusqlite::Result<()> {
    let name = name.trim();
    let old_id: Option<i64> = conn
        .query_row("SELECT publisher FROM books_publishers_link WHERE book = ?1", [book_id], |row| row.get(0))
        .optional()?;
    if name.is_empty() {
        conn.execute("DELETE FROM books_publishers_link WHERE book = ?1", [book_id])?;
    } else {
        let existing: Option<(i64, String)> = conn
            .query_row(
                "SELECT id, name FROM publishers WHERE name = ?1 COLLATE NOCASE",
                [name],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let new_id = match existing {
            Some((id, current)) => {
                if current != name {
                    conn.execute(
                        "UPDATE publishers SET name = ?1 WHERE id = ?2",
                        rusqlite::params![name, id],
                    )?;
                }
                id
            }
            None => {
                conn.execute("INSERT INTO publishers (name) VALUES (?1)", [name])?;
                conn.last_insert_rowid()
            }
        };
        if old_id != Some(new_id) {
            conn.execute("DELETE FROM books_publishers_link WHERE book = ?1", [book_id])?;
            conn.execute(
                "INSERT INTO books_publishers_link (book, publisher) VALUES (?1, ?2)",
                [book_id, new_id],
            )?;
        }
    }
    if let Some(old) = old_id {
        conn.execute(
            "DELETE FROM publishers WHERE id = ?1 AND NOT EXISTS (SELECT 1 FROM books_publishers_link WHERE publisher = ?1)",
            [old],
        )?;
    }
    Ok(())
}

// Replaces the book's whole identifier list. One value per type, types lowercased.
fn apply_identifiers(conn: &Connection, book_id: i64, items: &[IdentifierEdit]) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM identifiers WHERE book = ?1", [book_id])?;
    let mut seen: Vec<String> = Vec::new();
    for it in items {
        let typ = it.kind.trim().to_lowercase();
        let val = it.val.trim();
        if typ.is_empty() || val.is_empty() || seen.contains(&typ) {
            continue;
        }
        conn.execute(
            "INSERT INTO identifiers (book, type, val) VALUES (?1, ?2, ?3)",
            rusqlite::params![book_id, typ, val],
        )?;
        seen.push(typ);
    }
    Ok(())
}

#[derive(Serialize)]
pub struct EditExtras {
    pub identifiers: Vec<(String, String)>,
    pub publisher: Option<String>,
    pub publishers: Vec<String>,
    pub languages: Vec<String>,
    pub language_options: Vec<LanguageOption>,
    pub timestamp: String,
    pub pubdate: String,
}

pub fn edit_extras(conn: &Connection, book_id: i64) -> rusqlite::Result<EditExtras> {
    let (timestamp, pubdate): (String, String) = conn.query_row(
        "SELECT COALESCE(timestamp, ''), COALESCE(pubdate, '') FROM books WHERE id = ?1",
        [book_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let publisher: Option<String> = conn
        .query_row(
            "SELECT p.name FROM publishers p JOIN books_publishers_link l ON l.publisher = p.id WHERE l.book = ?1",
            [book_id],
            |row| row.get(0),
        )
        .optional()?;
    let identifiers = {
        let mut stmt = conn.prepare("SELECT type, val FROM identifiers WHERE book = ?1 ORDER BY id")?;
        let rows = stmt.query_map([book_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let found = rows.collect::<Result<Vec<_>, _>>()?;
        found
    };
    let publishers = query_strings(conn, "SELECT name FROM publishers ORDER BY name", rusqlite::params![])?;
    let languages = query_strings(
        conn,
        "SELECT g.lang_code FROM languages g JOIN books_languages_link l ON l.lang_code = g.id \
            WHERE l.book = ?1 ORDER BY l.item_order",
        rusqlite::params![book_id],
    )?;
    Ok(EditExtras {
        identifiers,
        publisher,
        publishers,
        timestamp,
        pubdate,
        languages,
        language_options: language_options(),
    })
}

#[derive(Serialize)]
pub struct LanguageOption {
    pub code: String,
    pub name: String,
}

// The common languages (those with a two letter code) and their three letter codes. Calibre
// writes ISO 639-2/T codes, which match these for every language in this list.
pub fn language_options() -> Vec<LanguageOption> {
    let mut out: Vec<LanguageOption> = isolang::languages()
        .filter(|l| l.to_639_1().is_some())
        .map(|l| LanguageOption {
            code: l.to_639_3().to_string(),
            name: l.to_name().to_string(),
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

pub fn valid_language_code(code: &str) -> bool {
    isolang::Language::from_639_3(&code.to_lowercase()).is_some()
}

// Languages are shared rows keyed by their code, and the link table keeps the order.
fn apply_languages(conn: &Connection, book_id: i64, codes: &[String]) -> rusqlite::Result<()> {
    let mut wanted: Vec<String> = Vec::new();
    for c in codes {
        let c = c.trim().to_lowercase();
        if !c.is_empty() && !wanted.contains(&c) {
            wanted.push(c);
        }
    }
    let old_ids: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT lang_code FROM books_languages_link WHERE book = ?1")?;
        let rows = stmt.query_map([book_id], |row| row.get::<_, i64>(0))?;
        let found = rows.collect::<Result<Vec<i64>, _>>()?;
        found
    };
    conn.execute("DELETE FROM books_languages_link WHERE book = ?1", [book_id])?;
    let mut new_ids: Vec<i64> = Vec::new();
    for (order, code) in wanted.iter().enumerate() {
        let existing: Option<i64> = conn
            .query_row(
                "SELECT id FROM languages WHERE lang_code = ?1 COLLATE NOCASE",
                [code],
                |row| row.get(0),
            )
            .optional()?;
        let id = match existing {
            Some(id) => id,
            None => {
                conn.execute("INSERT INTO languages (lang_code) VALUES (?1)", [code])?;
                conn.last_insert_rowid()
            }
        };
        conn.execute(
            "INSERT INTO books_languages_link (book, lang_code, item_order) VALUES (?1, ?2, ?3)",
            rusqlite::params![book_id, id, order as i64],
        )?;
        new_ids.push(id);
    }
    // Same cleanup as tags and series. Whether calibre also drops unused languages is
    // one of the tests below.
    for id in old_ids.iter().filter(|id| !new_ids.contains(id)) {
        conn.execute(
            "DELETE FROM languages WHERE id = ?1 AND NOT EXISTS (SELECT 1 FROM books_languages_link WHERE lang_code = ?1)",
            [*id],
        )?;
    }
    Ok(())
}

// Comments, rating, tags and series, which never touch the disk.

fn apply_basic_fields(tx: &Connection, book_id: i64, edit: &MetadataEdit) -> rusqlite::Result<()> {
    if let Some(name) = &edit.publisher {
        apply_publisher(tx, book_id, name)?;
    }
    if let Some(items) = &edit.identifiers {
        apply_identifiers(tx, book_id, items)?;
    }
    if let Some(codes) = &edit.languages {
        apply_languages(tx, book_id, codes)?;
    }
    if let Some(d) = &edit.date_added {
        tx.execute("UPDATE books SET timestamp = ?1 WHERE id = ?2", rusqlite::params![d, book_id])?;
    }
    if let Some(d) = &edit.published {
        tx.execute("UPDATE books SET pubdate = ?1 WHERE id = ?2", rusqlite::params![d, book_id])?;
    }
    if let Some(text) = &edit.comments {
        apply_comments(&tx, book_id, text)?;
    }
    if let Some(stars) = edit.rating {
        apply_rating(&tx, book_id, stars)?;
    }
    if let Some(tags) = &edit.tags {
        apply_tags(&tx, book_id, tags)?;
    }
    match (&edit.series, edit.series_index) {
        (Some(name), idx) => apply_series(&tx, book_id, name, idx)?,
        (None, Some(idx)) => {
            // Index changed on its own. Only meaningful if the book has a series.
            let has_series: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM books_series_link WHERE book = ?1)",
                [book_id],
                |row| row.get(0),
            )?;
            if has_series {
                tx.execute(
                    "UPDATE books SET series_index = ?1 WHERE id = ?2",
                    rusqlite::params![idx, book_id],
                )?;
            }
        }
        (None, None) => {}
    }
    tx.execute(
        "UPDATE books SET last_modified = strftime('%Y-%m-%d %H:%M:%f+00:00', 'now') WHERE id = ?1",
        [book_id],
    )?;
    Ok(())
}

// Empty text removes the comments row, which is what calibre does.
fn apply_comments(conn: &Connection, book_id: i64, text: &str) -> rusqlite::Result<()> {
    if text.trim().is_empty() {
        conn.execute("DELETE FROM comments WHERE book = ?1", [book_id])?;
    } else {
        let updated = conn.execute(
            "UPDATE comments SET text = ?1 WHERE book = ?2",
            rusqlite::params![text, book_id],
        )?;
        if updated == 0 {
            conn.execute(
                "INSERT INTO comments (book, text) VALUES (?1, ?2)",
                rusqlite::params![book_id, text],
            )?;
        }
    }
    Ok(())
}

fn apply_tags(conn: &Connection, book_id: i64, tags: &[String]) -> rusqlite::Result<()> {
    // Trim, drop empties, and dedupe ignoring ASCII case, which is how the tags table compares names.
    let mut wanted: Vec<String> = Vec::new();
    for t in tags {
        let t = t.trim();
        if !t.is_empty() && !wanted.iter().any(|w| w.eq_ignore_ascii_case(t)) {
            wanted.push(t.to_string());
        }
    }

    let old_ids: Vec<i64> = {
        let mut stmt = conn.prepare("SELECT tag FROM books_tags_link WHERE book = ?1")?;
        let rows = stmt.query_map([book_id], |row| row.get(0))?;
        rows.collect::<Result<_, _>>()?
    };

    // Reuse an existing tag row when one matches ignoring case (renaming it to the new
    // spelling, like calibre does), otherwise create it.
    let mut new_ids: Vec<i64> = Vec::new();
    for name in &wanted {
        let existing: Option<(i64, String)> = conn
            .query_row(
                "SELECT id, name FROM tags WHERE name = ?1 COLLATE NOCASE",
                [name],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let id = match existing {
            Some((id, current)) => {
                // Calibre renames the shared row when the capitalization differs,
                // so every book with this tag shows the new spelling.
                if current != *name {
                    conn.execute(
                        "UPDATE tags SET name = ?1 WHERE id = ?2",
                        rusqlite::params![name, id],
                    )?;
                }
                id
            }
            None => {
                conn.execute("INSERT INTO tags (name) VALUES (?1)", [name])?;
                conn.last_insert_rowid()
            }
        };
        if !new_ids.contains(&id) {
            new_ids.push(id);
        }
    }

    for id in old_ids.iter().filter(|id| !new_ids.contains(id)) {
        conn.execute(
            "DELETE FROM books_tags_link WHERE book = ?1 AND tag = ?2",
            [book_id, *id],
        )?;
        // Calibre drops tags nothing uses anymore. Only tags this edit un-linked are checked.
        conn.execute(
            "DELETE FROM tags WHERE id = ?1 AND NOT EXISTS (SELECT 1 FROM books_tags_link WHERE tag = ?1)",
            [*id],
        )?;
    }
    for id in new_ids.iter().filter(|id| !old_ids.contains(id)) {
        conn.execute(
            "INSERT INTO books_tags_link (book, tag) VALUES (?1, ?2)",
            [book_id, *id],
        )?;
    }
    Ok(())
}

// Moves this book to the named series. If the name matches an existing series ignoring
// case, that shared row takes the new spelling for every book, as calibre does.
fn apply_series(conn: &Connection, book_id: i64, series: &str, index: Option<f64>) -> rusqlite::Result<()> {
    let name = series.trim();
    let old_id: Option<i64> = conn
        .query_row("SELECT series FROM books_series_link WHERE book = ?1", [book_id], |row| row.get(0))
        .optional()?;

    if name.is_empty() {
        conn.execute("DELETE FROM books_series_link WHERE book = ?1", [book_id])?;
        conn.execute("UPDATE books SET series_index = 1.0 WHERE id = ?1", [book_id])?;
    } else {
        let existing: Option<(i64, String)> = conn
            .query_row(
                "SELECT id, name FROM series WHERE name = ?1 COLLATE NOCASE",
                [name],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let new_id = match existing {
            Some((id, current)) => {
                // Same as tags: calibre renames the shared row when only the
                // capitalization differs. series_update_trg refreshes the sort value.
                if current != name {
                    conn.execute(
                        "UPDATE series SET name = ?1 WHERE id = ?2",
                        rusqlite::params![name, id],
                    )?;
                }
                id
            }
            // The series_insert_trg trigger fills in the sort value.
            None => {
                conn.execute("INSERT INTO series (name) VALUES (?1)", [name])?;
                conn.last_insert_rowid()
            }
        };
        if old_id != Some(new_id) {
            conn.execute("DELETE FROM books_series_link WHERE book = ?1", [book_id])?;
            conn.execute(
                "INSERT INTO books_series_link (book, series) VALUES (?1, ?2)",
                [book_id, new_id],
            )?;
        }
        if let Some(i) = index {
            conn.execute(
                "UPDATE books SET series_index = ?1 WHERE id = ?2",
                rusqlite::params![i, book_id],
            )?;
        }
    }

    // Calibre drops series nothing uses anymore. Only the series this edit left is checked.
    if let Some(old) = old_id {
        conn.execute(
            "DELETE FROM series WHERE id = ?1 AND NOT EXISTS (SELECT 1 FROM books_series_link WHERE series = ?1)",
            [old],
        )?;
    }
    Ok(())
}

// Next whole number after the highest index in that series, or 1 for a new series.
pub fn next_series_index(conn: &Connection, series: &str) -> rusqlite::Result<f64> {
    let max: Option<f64> = conn.query_row(
        "SELECT MAX(b.series_index) FROM books b
JOIN books_series_link l ON l.book = b.id
JOIN series s ON s.id = l.series
WHERE s.name = ?1 COLLATE NOCASE",
        [series.trim()],
        |row| row.get(0),
    )?;
    Ok(match max {
        Some(m) => m.floor() + 1.0,
        None => 1.0,
    })
}

// Calibre stores stars times two (0 to 10), with one shared row per value in `ratings`.
// No stars means no link row at all.
fn apply_rating(conn: &Connection, book_id: i64, stars: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM books_ratings_link WHERE book = ?1", [book_id])?;
    if stars > 0 {
        let value = stars * 2;
        let existing: Option<i64> = conn
            .query_row("SELECT id FROM ratings WHERE rating = ?1", [value], |row| row.get(0))
            .optional()?;
        let rating_id = match existing {
            Some(id) => id,
            None => {
                conn.execute("INSERT INTO ratings (rating) VALUES (?1)", [value])?;
                conn.last_insert_rowid()
            }
        };
        conn.execute(
            "INSERT INTO books_ratings_link (book, rating) VALUES (?1, ?2)",
            [book_id, rating_id],
        )?;
    }
    Ok(())
}

pub fn get_comments(conn: &Connection, book_id: i64) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT text FROM comments WHERE book = ?1",
        [book_id],
        |row| row.get(0),
    ).optional()
}

pub fn list_books(conn: &Connection, library_path: &str, filter: &BookFilter) -> rusqlite::Result<Vec<Book>> {
    let mut sql = String::from(
        "SELECT DISTINCT
            b.id, b.title, b.path, b.has_cover, b.series_index,
            (SELECT group_concat(a.name, ' & ')
            FROM authors a
            JOIN books_authors_link bal ON bal.author = a.id
            WHERE bal.book = b.id) as authors,
            (SELECT s.name FROM series s
            JOIN books_series_link bsl ON bsl.series = s.id
            WHERE bsl.book = b.id) as series,
            (SELECT group_concat(t.name, '\u{001f}')
            FROM tags t
            JOIN books_tags_link btl ON btl.tag = t.id
            WHERE btl.book = b.id) as tags_concat,
            (SELECT r.rating FROM ratings r
            JOIN books_ratings_link brl ON brl.rating = r.id
            WHERE brl.book = b.id) as rating
            FROM books b"
    );

    let mut conditions: Vec<String> = vec![];
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

    if let Some(id) = filter.author_id {
        sql.push_str(" JOIN books_authors_link f_bal ON f_bal.book = b.id");
        conditions.push("f_bal.author = ?".into());
        params.push(Box::new(id));
    }
    if let Some(id) = filter.series_id {
        sql.push_str(" JOIN books_series_link f_bsl ON f_bsl.book = b.id");
        conditions.push("f_bsl.series = ?".into());
        params.push(Box::new(id));
    }
    if let Some(id) = filter.tag_id {
        sql.push_str(" JOIN books_tags_link f_btl ON f_btl.book = b.id");
        conditions.push("f_btl.tag = ?".into());
        params.push(Box::new(id));
    }
    if let (Some(col_id), Some(val_id)) = (filter.custom_column_id, filter.custom_value_id) {
        sql.push_str(&format!(" JOIN books_custom_column_{col_id}_link f_cc ON f_cc.book = b.id"));
        conditions.push("f_cc.value = ?".into());
        params.push(Box::new(val_id));
    }
    if let Some(ref s) = filter.search {
        conditions.push(
            "(b.title LIKE ? \
                OR EXISTS (SELECT 1 FROM books_authors_link bal2 \
                JOIN authors a2 ON a2.id = bal2.author \
                WHERE bal2.book = b.id AND replace(a2.name, '|', ',') LIKE ?) \
                OR EXISTS (SELECT 1 FROM books_tags_link btl2 \
                JOIN tags t2 ON t2.id = btl2.tag \
                WHERE btl2.book = b.id AND t2.name LIKE ?))".into()
        );
        let pattern = format!("%{}%", s);
        params.push(Box::new(pattern.clone()));
        params.push(Box::new(pattern.clone()));
        params.push(Box::new(pattern));
    }

    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    sql.push_str(" ORDER BY b.sort");

    let mut stmt = conn.prepare(&sql)?;
    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();

    let book_iter = stmt.query_map(param_refs.as_slice(), |row| {
        let id: i64 = row.get(0)?;
        let path: String = row.get(2)?;
        let has_cover = row.get::<_, i64>(3)? != 0;
        let cover_path = resolve_cover(library_path, &path, has_cover);
        let cover_version = cover_path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let authors = row.get::<_, Option<String>>(5)?
            .map(|s| restore_author_name(&s))
            .unwrap_or_default();
        let tags_concat: Option<String> = row.get(7)?;
        let tags = tags_concat
            .map(|s| s.split('\u{1f}').map(String::from).collect())
            .unwrap_or_default();

        Ok(Book {
            id,
            title: row.get(1)?,
            path,
            has_cover,
            series_index: row.get(4)?,
            authors,
            series: row.get(6)?,
            rating: row.get(8)?,
            cover_path,
            cover_version,
            tags,
        })
    })?;

    book_iter.collect()
}
