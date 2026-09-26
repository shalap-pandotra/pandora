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
    pub comments: Option<String>,
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
    pub values: Vec<GenreRow>, // reusing the same shape, id/value/count
}

#[derive(Serialize)]
pub struct GenreRow {
    pub id: i64,
    pub value: String,
    pub count: i64,
}

pub fn list_multi_text_columns(conn: &Connection) -> rusqlite::Result<Vec<MultiTextColumn>> {
    let mut col_stmt = conn.prepare(
        "SELECT id, label, name FROM custom_columns
WHERE datatype = 'text' AND is_multiple = 1"
    )?;

    let columns: Vec<(i64, String, String)> = col_stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
        .collect::<Result<_, _>>()?;

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

        result.push(MultiTextColumn { id: col_id, label, name, values });
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
        Ok(FilterRow { id: row.get(0)?, name: row.get(1)?, count: row.get(2)? })
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

pub fn open_library(library_path: &str) -> rusqlite::Result<Connection> {
    let db_path = PathBuf::from(library_path).join("metadata.db");
    // Open read-only — phase 1 never writes
    Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
}

pub fn list_books(conn: &Connection, filter: &BookFilter) -> rusqlite::Result<Vec<Book>> {
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
(SELECT text FROM comments WHERE comments.book = b.id) as comments
FROM books b"
    );
    let mut conditions: Vec<String> = vec![];
    if filter.author_id.is_some() {
        sql.push_str(" JOIN books_authors_link f_bal ON f_bal.book = b.id");
        conditions.push("f_bal.author = ?".into());
    }
    if let (Some(col_id), Some(_)) = (filter.custom_column_id, filter.custom_value_id) {
        sql.push_str(&format!(
        " JOIN books_custom_column_{col_id}_link f_cc ON f_cc.book = b.id"
    ));
        conditions.push("f_cc.value = ?".into());
    }
    if filter.series_id.is_some() {
        sql.push_str(" JOIN books_series_link f_bsl ON f_bsl.book = b.id");
        conditions.push("f_bsl.series = ?".into());
    }
    if filter.tag_id.is_some() {
        sql.push_str(" JOIN books_tags_link f_btl ON f_btl.book = b.id");
        conditions.push("f_btl.tag = ?".into());
    }


    if filter.search.is_some() {
        conditions.push(
            "(b.title LIKE ? \
                OR EXISTS (SELECT 1 FROM books_authors_link bal2 \
                JOIN authors a2 ON a2.id = bal2.author \
                WHERE bal2.book = b.id AND a2.name LIKE ?) \
                OR EXISTS (SELECT 1 FROM books_tags_link btl2 \
                JOIN tags t2 ON t2.id = btl2.tag \
                WHERE btl2.book = b.id AND t2.name LIKE ?))".into()
        );
    }

    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    sql.push_str(" ORDER BY b.title");

    let mut stmt = conn.prepare(&sql)?;

    // Bind params in the same order conditions were pushed
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];
    if let Some(id) = filter.author_id { params.push(Box::new(id)); }
    if let Some(id) = filter.series_id { params.push(Box::new(id)); }
    if let Some(id) = filter.tag_id { params.push(Box::new(id)); }
    if let Some(ref s) = filter.search {
        let pattern = format!("%{}%", s);
        params.push(Box::new(pattern.clone()));
        params.push(Box::new(pattern.clone()));
        params.push(Box::new(pattern));
    }
    if let Some(val_id) = filter.custom_value_id { params.push(Box::new(val_id)); }
    let param_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|b| b.as_ref()).collect();

    let book_iter = stmt.query_map(param_refs.as_slice(), |row| {
        let id: i64 = row.get(0)?;
        Ok(Book {
            id,
            title: row.get(1)?,
            path: row.get(2)?,
            has_cover: row.get::<_, i64>(3)? != 0,
            series_index: row.get(4)?,
            authors: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
            series: row.get(6)?,
            comments: row.get(7)?,
            tags: vec![],
        })
    })?;

    let mut books: Vec<Book> = book_iter.collect::<Result<_, _>>()?;

    for book in &mut books {
        let mut tag_stmt = conn.prepare(
            "SELECT t.name FROM tags t
JOIN books_tags_link btl ON btl.tag = t.id
WHERE btl.book = ?1"
        )?;
        let tags: Vec<String> = tag_stmt
            .query_map([book.id], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        book.tags = tags;
    }

    Ok(books)
}
