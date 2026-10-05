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
    pub rating: Option<i64>,
}

#[derive(Serialize)]
pub struct FilterRow {
    pub id: i64,
    pub name: String,
    pub count: i64,
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
WHERE datatype = 'text' AND is_multiple = 1"
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
        "SELECT a.id, a.name, COUNT(bal.book) as cnt
FROM authors a
JOIN books_authors_link bal ON bal.author = a.id
GROUP BY a.id
ORDER BY cnt DESC, a.name ASC"
    )?;

    let rows = stmt.query_map([], |row| {
        let name: String = row.get(1)?;
        Ok(FilterRow { id: row.get(0)?, name: restore_author_name(&name), count: row.get(2)? })
    })?;
    rows.collect()
}
pub fn list_series(conn: &Connection) -> rusqlite::Result<Vec<FilterRow>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.name, COUNT(bsl.book) as cnt
FROM series s
JOIN books_series_link bsl ON bsl.series = s.id
GROUP BY s.id
ORDER BY cnt DESC, s.name ASC"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(FilterRow { id: row.get(0)?, name: row.get(1)?, count: row.get(2)? })
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
        Ok(FilterRow { id: row.get(0)?, name: row.get(1)?, count: row.get(2)? })
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

fn resolve_cover(library_path: &str, book_path: &str, has_cover: bool) -> Option<String> {
    if !has_cover { return None; }
    let cover = std::path::Path::new(library_path).join(book_path).join("cover.jpg");
    cover.exists().then(|| cover.to_string_lossy().to_string())
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
    pub tags: Option<Vec<String>>, // None means unchanged
    pub series: Option<String>,    // None means unchanged, empty string clears
    pub series_index: Option<f64>, // None means unchanged
    pub comments: Option<String>, // None means unchanged
    pub rating: Option<i64>,      // None means unchanged, 0 clears, 1 to 5 sets stars
}

// Each field only runs if it was actually changed. One transaction covers all of
// them, so a failure part way through leaves the library untouched.
pub fn write_metadata(conn: &mut Connection, book_id: i64, edit: &MetadataEdit) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
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
    tx.commit()
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
            tags,
        })
    })?;

    book_iter.collect()
}
