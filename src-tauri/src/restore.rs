//! Putting a library back the way it was before Pandora changed it.
//!
//! Before its first write of a session Pandora copies metadata.db into its backups folder, and every
//! folder or file it renames is appended to a log next to those copies. A restore brings a copy
//! back and walks the renames made since then backwards, so the database and the folders on disk
//! agree again. Everything is done so that a failure part way leaves the library as it was.

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// One backup the library can be restored to.
#[derive(Serialize, Clone, Debug)]
pub struct BackupInfo {
    /// The file's name in the backups folder.
    pub file: String,
    /// When it was taken, in seconds since 1970.
    pub taken: u64,
    pub size: u64,
    /// True for the copy a restore makes of the library as it was just before restoring.
    pub before_restore: bool,
    /// How many folder or file renames made since would be undone.
    pub moves: usize,
}

#[derive(Serialize, Debug)]
pub struct RestoreReport {
    pub moves_undone: usize,
    /// The copy of the library as it was just before this restore.
    pub safety_copy: String,
    /// Books in the restored library whose folder is not on disk.
    pub missing_folders: usize,
    pub missing_examples: Vec<String>,
}

struct Move {
    stamp: u64,
    from: String,
    to: String,
}

pub fn library_name(library: &Path) -> String {
    library
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "library".into())
}

fn log_path(backups: &Path, name: &str) -> PathBuf {
    backups.join(format!("{}-renames.log", name))
}

/// `{name}-{secs}.db` is a backup taken before a session's first save, and
/// `{name}-before-restore-{secs}.db` is the copy a restore makes first.
fn parse_backup(file: &str, name: &str) -> Option<(u64, bool)> {
    let rest = file.strip_prefix(name)?.strip_prefix('-')?.strip_suffix(".db")?;
    if let Some(secs) = rest.strip_prefix("before-restore-") {
        return secs.parse().ok().map(|s| (s, true));
    }
    rest.parse().ok().map(|s| (s, false))
}

/// The id calibre gives each library, so a backup of another library that happens to have the same
/// folder name is never mistaken for this one.
fn library_uuid(db: &Path) -> Option<String> {
    let conn = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    conn.query_row("SELECT uuid FROM library_id LIMIT 1", [], |r| r.get::<_, String>(0)).ok()
}

fn is_sound(db: &Path) -> bool {
    let Ok(conn) = Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY) else { return false };
    conn.query_row("PRAGMA quick_check", [], |r| r.get::<_, String>(0)).map(|s| s == "ok").unwrap_or(false)
}

fn read_moves(log: &Path) -> Vec<Move> {
    let Ok(text) = fs::read_to_string(log) else { return Vec::new() };
    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let stamp = parts.next()?.parse().ok()?;
            let from = parts.next()?;
            let to = parts.next()?;
            if from.is_empty() || to.is_empty() {
                return None;
            }
            Some(Move { stamp, from: from.to_string(), to: to.to_string() })
        })
        .collect()
}

/// A path from the log has to stay inside the library.
fn stays_inside(rel: &str) -> bool {
    !rel.is_empty() && Path::new(rel).components().all(|c| matches!(c, Component::Normal(_)))
}

pub fn list_backups(backups: &Path, library: &Path) -> Vec<BackupInfo> {
    let name = library_name(library);
    let ours = library_uuid(&library.join("metadata.db"));
    let moves = read_moves(&log_path(backups, &name));
    let Ok(entries) = fs::read_dir(backups) else { return Vec::new() };
    let mut found: Vec<BackupInfo> = entries
        .filter_map(|e| e.ok())
        .filter_map(|entry| {
            let file = entry.file_name().to_string_lossy().to_string();
            let (taken, before_restore) = parse_backup(&file, &name)?;
            let path = entry.path();
            if !is_sound(&path) {
                return None;
            }
            if let (Some(a), Some(b)) = (&ours, library_uuid(&path)) {
                if *a != b {
                    return None;
                }
            }
            let size = entry.metadata().ok()?.len();
            Some(BackupInfo {
                file,
                taken,
                size,
                before_restore,
                moves: moves.iter().filter(|m| m.stamp >= taken).count(),
            })
        })
        .collect();
    found.sort_by(|a, b| b.taken.cmp(&a.taken));
    found
}

/// Moves `from` to `to` (both relative to the library). Never overwrites anything. A change in
/// capitalization only goes through a temporary name, so it works on Windows.
fn move_back(library: &Path, from: &str, to: &str) -> Result<(), String> {
    let from_p = library.join(from);
    let to_p = library.join(to);
    if !from_p.exists() {
        return Err(format!("{} is not there any more", from));
    }
    let case_only = from != to && from.to_lowercase() == to.to_lowercase();
    if case_only {
        let mut tmp = from_p.as_os_str().to_owned();
        tmp.push(".pandora-tmp");
        let tmp = PathBuf::from(tmp);
        fs::rename(&from_p, &tmp).map_err(|e| format!("Could not rename {}: {}", from, e))?;
        if let Err(e) = fs::rename(&tmp, &to_p) {
            let _ = fs::rename(&tmp, &from_p);
            return Err(format!("Could not rename {}: {}", from, e));
        }
    } else {
        if to_p.exists() {
            return Err(format!("{} already exists", to));
        }
        if let Some(parent) = to_p.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("Could not create {}: {}", parent.display(), e))?;
        }
        fs::rename(&from_p, &to_p).map_err(|e| format!("Could not move {}: {}", from, e))?;
    }
    // A folder the move emptied, such as an author's, is removed, as calibre does. Only an empty
    // folder can be removed this way, and never the library itself.
    if let Some(parent) = from_p.parent() {
        if parent != library {
            let _ = fs::remove_dir(parent);
        }
    }
    Ok(())
}

/// Undoes the logged moves, newest first. If one cannot be undone, the ones already done are put
/// back and nothing is changed. Returns the moves as carried out, as (from, to) pairs.
fn undo_moves(library: &Path, moves: &[Move]) -> Result<Vec<(String, String)>, String> {
    let mut done: Vec<(String, String)> = Vec::new();
    for m in moves.iter().rev() {
        // The log says `from` became `to`, so going back `to` becomes `from`.
        match move_back(library, &m.to, &m.from) {
            Ok(()) => done.push((m.to.clone(), m.from.clone())),
            Err(e) => {
                for (a, b) in done.iter().rev() {
                    let _ = move_back(library, b, a);
                }
                return Err(e);
            }
        }
    }
    Ok(done)
}

fn append_moves(backups: &Path, name: &str, stamp: u64, moves: &[(String, String)]) {
    use std::io::Write;
    if moves.is_empty() {
        return;
    }
    let mut text = String::new();
    for (from, to) in moves {
        text.push_str(&format!("{}\t{}\t{}\n", stamp, from, to));
    }
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(log_path(backups, name)) {
        let _ = f.write_all(text.as_bytes());
    }
}

/// Books in the library whose folder is not on disk.
fn missing_folders(library: &Path) -> (usize, Vec<String>) {
    let Ok(conn) = Connection::open_with_flags(library.join("metadata.db"), OpenFlags::SQLITE_OPEN_READ_ONLY) else {
        return (0, Vec::new());
    };
    let Ok(mut stmt) = conn.prepare("SELECT path FROM books") else { return (0, Vec::new()) };
    let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(0)) else { return (0, Vec::new()) };
    let mut count = 0;
    let mut examples = Vec::new();
    for path in rows.flatten() {
        if !path.is_empty() && !library.join(&path).is_dir() {
            count += 1;
            if examples.len() < 5 {
                examples.push(path);
            }
        }
    }
    (count, examples)
}

/// Restores `file` (one of this library's backups) and undoes the folder and file renames made
/// since it was taken. The caller must have let go of its own open copy of metadata.db first.
pub fn restore(backups: &Path, library: &Path, file: &str, now: u64) -> Result<RestoreReport, String> {
    let name = library_name(library);
    let not_ours = || "That is not one of this library's backups".to_string();
    if file.contains('/') || file.contains('\\') || file.contains("..") {
        return Err(not_ours());
    }
    let (taken, _) = parse_backup(file, &name).ok_or_else(not_ours)?;
    let snapshot = backups.join(file);
    if !snapshot.is_file() {
        return Err("That backup is no longer there".to_string());
    }
    let db = library.join("metadata.db");
    if let (Some(a), Some(b)) = (library_uuid(&db), library_uuid(&snapshot)) {
        if a != b {
            return Err("That backup belongs to a different library".to_string());
        }
    }
    if !is_sound(&snapshot) {
        return Err("That backup is damaged and cannot be used".to_string());
    }

    // The renames to walk back: everything logged since the backup was taken.
    let moves: Vec<Move> = read_moves(&log_path(backups, &name)).into_iter().filter(|m| m.stamp >= taken).collect();
    if moves.iter().any(|m| !stays_inside(&m.from) || !stays_inside(&m.to)) {
        return Err("The rename log names a path outside the library, so nothing was changed".to_string());
    }

    // A copy of the library as it is now, so that this restore can itself be undone.
    let safety = format!("{}-before-restore-{}.db", name, now);
    let safety_path = backups.join(&safety);
    fs::copy(&db, &safety_path).map_err(|e| format!("Could not copy the current database first: {}", e))?;
    let give_up = |message: String| -> String {
        let _ = fs::remove_file(&safety_path);
        message
    };

    // Stage the backup next to the database, then swap it in. If the database is in use (calibre
    // has it open), the swap is what fails, before anything else has been touched.
    let staged = library.join("metadata.db.pandora-restore");
    let old = library.join("metadata.db.pandora-old");
    let _ = fs::remove_file(&staged);
    let _ = fs::remove_file(&old);
    fs::copy(&snapshot, &staged).map_err(|e| give_up(format!("Could not copy the backup into the library: {}", e)))?;
    if let Err(e) = fs::rename(&db, &old) {
        let _ = fs::remove_file(&staged);
        return Err(give_up(format!(
            "metadata.db could not be replaced. Is calibre, or anything else, using the library? ({})",
            e
        )));
    }
    if let Err(e) = fs::rename(&staged, &db) {
        let _ = fs::rename(&old, &db);
        let _ = fs::remove_file(&staged);
        return Err(give_up(format!("Could not put the backup in place: {}", e)));
    }

    // Now the folders and files go back.
    let done = match undo_moves(library, &moves) {
        Ok(done) => done,
        Err(e) => {
            let _ = fs::remove_file(&db);
            let _ = fs::rename(&old, &db);
            return Err(give_up(format!("{}. Nothing was changed.", e)));
        }
    };
    let _ = fs::remove_file(&old);
    // Leftovers of the replaced database must not be applied to the restored one.
    for leftover in ["metadata.db-journal", "metadata.db-wal", "metadata.db-shm"] {
        let _ = fs::remove_file(library.join(leftover));
    }

    // Recorded as ordinary renames, so restoring the safety copy later undoes them again.
    append_moves(backups, &name, now, &done);
    let (missing, examples) = missing_folders(library);
    Ok(RestoreReport { moves_undone: done.len(), safety_copy: safety, missing_folders: missing, missing_examples: examples })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static N: AtomicU32 = AtomicU32::new(0);
    const LIB: &str = "Calibre Library";

    struct Env { root: PathBuf, lib: PathBuf, backups: PathBuf }
    impl Env {
        fn new() -> Env {
            let root = std::env::temp_dir().join(format!("pandora-restore-test-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
            let _ = fs::remove_dir_all(&root);
            let lib = root.join(LIB);
            let backups = root.join("backups");
            fs::create_dir_all(&lib).unwrap();
            fs::create_dir_all(&backups).unwrap();
            Env { root, lib, backups }
        }
        fn db(&self) -> PathBuf { self.lib.join("metadata.db") }
        fn path(&self, rel: &str) -> PathBuf { self.lib.join(rel) }
        fn exists(&self, rel: &str) -> bool { self.path(rel).exists() }
    }
    impl Drop for Env { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.root); } }

    fn make_db(path: &Path, uuid: &str, books: &[(i64, &str)]) {
        let _ = fs::remove_file(path);
        let c = Connection::open(path).unwrap();
        c.execute_batch("CREATE TABLE library_id (id INTEGER PRIMARY KEY, uuid TEXT NOT NULL); CREATE TABLE books (id INTEGER PRIMARY KEY, path TEXT NOT NULL);").unwrap();
        c.execute("INSERT INTO library_id (uuid) VALUES (?1)", [uuid]).unwrap();
        for (id, p) in books { c.execute("INSERT INTO books (id, path) VALUES (?1, ?2)", rusqlite::params![id, p]).unwrap(); }
    }
    fn paths(db: &Path) -> Vec<(i64, String)> {
        let c = Connection::open(db).unwrap();
        let mut s = c.prepare("SELECT id, path FROM books ORDER BY id").unwrap();
        s.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().map(|r| r.unwrap()).collect()
    }
    fn touch(p: &Path) { fs::create_dir_all(p.parent().unwrap()).unwrap(); fs::write(p, b"x").unwrap(); }
    fn log(env: &Env, lines: &[(u64, &str, &str)]) {
        use std::io::Write;
        let mut f = fs::OpenOptions::new().create(true).append(true).open(env.backups.join(format!("{}-renames.log", LIB))).unwrap();
        for (s, a, b) in lines { writeln!(f, "{}\t{}\t{}", s, a, b).unwrap(); }
    }
    fn log_text(env: &Env) -> String { fs::read_to_string(env.backups.join(format!("{}-renames.log", LIB))).unwrap_or_default() }
    fn mv(env: &Env, from: &str, to: &str) { fs::create_dir_all(env.path(to).parent().unwrap()).unwrap(); fs::rename(env.path(from), env.path(to)).unwrap(); }

    const UUID: &str = "11111111-2222-3333-4444-555555555555";

    /// State A is the library as it was at stamp 1000, when the backup was taken. Then Pandora retitles
    /// book 1 (file renamed, then its folder), moves book 2 to a new author (the old author's folder is
    /// removed) and changes the capitalization of book 3's author folder. That is state B.
    fn edited(env: &Env) {
        for rel in ["Author A/Book One (1)", "Author B/Book Two (2)", "Author C/Book Three (3)"] {
            touch(&env.path(&format!("{}/title.epub", rel)));
            touch(&env.path(&format!("{}/cover.jpg", rel)));
        }
        touch(&env.path("Author A/Book One (1)/Book One - Author A.epub"));
        make_db(&env.db(), UUID, &[(1, "Author A/Book One (1)"), (2, "Author B/Book Two (2)"), (3, "Author C/Book Three (3)")]);
        fs::copy(env.db(), env.backups.join(format!("{}-1000.db", LIB))).unwrap();
        // an old rename, from before the backup, that a restore must leave alone
        touch(&env.path("Older/Thing (9)/a.epub"));
        log(env, &[(500, "Old/Thing (9)", "Older/Thing (9)")]);

        mv(env, "Author A/Book One (1)/Book One - Author A.epub", "Author A/Book One (1)/Book 1 - Author A.epub");
        mv(env, "Author A/Book One (1)", "Author A/Book 1 (1)");
        mv(env, "Author B/Book Two (2)", "Author X/Book Two (2)");
        let _ = fs::remove_dir(env.path("Author B"));
        mv(env, "Author C", "AUTHOR C");
        log(env, &[
            (2000, "Author A/Book One (1)/Book One - Author A.epub", "Author A/Book One (1)/Book 1 - Author A.epub"),
            (2000, "Author A/Book One (1)", "Author A/Book 1 (1)"),
            (2000, "Author B/Book Two (2)", "Author X/Book Two (2)"),
            (2000, "Author C", "AUTHOR C"),
        ]);
        make_db(&env.db(), UUID, &[(1, "Author A/Book 1 (1)"), (2, "Author X/Book Two (2)"), (3, "AUTHOR C/Book Three (3)")]);
    }
    fn state_a(env: &Env) {
        assert_eq!(paths(&env.db()), vec![(1, "Author A/Book One (1)".to_string()), (2, "Author B/Book Two (2)".to_string()), (3, "Author C/Book Three (3)".to_string())]);
        assert!(env.exists("Author A/Book One (1)/Book One - Author A.epub"));
        assert!(!env.exists("Author A/Book 1 (1)"));
        assert!(env.exists("Author B/Book Two (2)/title.epub"));
        assert!(!env.exists("Author X"), "the author folder the edit created is removed again");
        assert!(env.exists("Author C/Book Three (3)/cover.jpg"));
        assert!(env.exists("Older/Thing (9)/a.epub") && !env.exists("Old"), "a rename from before the backup is left alone");
    }
    fn state_b(env: &Env) {
        assert_eq!(paths(&env.db()), vec![(1, "Author A/Book 1 (1)".to_string()), (2, "Author X/Book Two (2)".to_string()), (3, "AUTHOR C/Book Three (3)".to_string())]);
        assert!(env.exists("Author A/Book 1 (1)/Book 1 - Author A.epub"));
        assert!(env.exists("Author X/Book Two (2)/title.epub") && !env.exists("Author B"));
        assert!(env.exists("AUTHOR C/Book Three (3)/cover.jpg"));
    }
    fn no_leftovers(env: &Env) {
        for f in ["metadata.db.pandora-restore", "metadata.db.pandora-old"] { assert!(!env.exists(f), "{} was left behind", f); }
    }
    fn names(env: &Env) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(&env.backups).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
        v.sort(); v
    }

    #[test]
    fn lists_backups_newest_first_with_the_moves_each_would_undo() {
        let env = Env::new(); edited(&env);
        fs::copy(env.db(), env.backups.join(format!("{}-1900.db", LIB))).unwrap();       // a later session's backup
        let list = list_backups(&env.backups, &env.lib);
        assert_eq!(list.iter().map(|b| b.taken).collect::<Vec<_>>(), vec![1900, 1000]);
        assert_eq!(list[0].moves, 4, "the 4 renames logged at 2000 are after the 1900 backup too");
        assert_eq!(list[1].moves, 4, "the old rename at 500 is before the backup and is not counted");
        assert!(!list[0].before_restore);
    }

    #[test]
    fn restores_the_database_and_puts_every_folder_back() {
        let env = Env::new(); edited(&env);
        let report = restore(&env.backups, &env.lib, "Calibre Library-1000.db", 3000).unwrap();
        state_a(&env); no_leftovers(&env);
        assert_eq!(report.moves_undone, 4);
        assert_eq!(report.missing_folders, 0);
        assert_eq!(report.safety_copy, "Calibre Library-before-restore-3000.db");
        assert!(env.backups.join(&report.safety_copy).is_file());
        // what the restore moved is logged, so it can be undone
        assert_eq!(log_text(&env).lines().filter(|l| l.starts_with("3000\t")).count(), 4);
    }

    #[test]
    fn a_restore_can_itself_be_undone() {
        let env = Env::new(); edited(&env);
        restore(&env.backups, &env.lib, "Calibre Library-1000.db", 3000).unwrap();
        state_a(&env);
        let list = list_backups(&env.backups, &env.lib);
        assert!(list.iter().any(|b| b.file == "Calibre Library-before-restore-3000.db" && b.before_restore));
        let again = restore(&env.backups, &env.lib, "Calibre Library-before-restore-3000.db", 4000).unwrap();
        assert_eq!(again.moves_undone, 4);
        state_b(&env); no_leftovers(&env);
        // and back again once more
        restore(&env.backups, &env.lib, "Calibre Library-before-restore-4000.db", 5000).unwrap();
        state_a(&env);
    }

    #[test]
    fn a_folder_in_the_way_changes_nothing_at_all() {
        let env = Env::new(); edited(&env);
        touch(&env.path("Author A/Book One (1)/something-the-user-made.txt"));       // the old name is taken
        let before = names(&env);
        let err = restore(&env.backups, &env.lib, "Calibre Library-1000.db", 3000).unwrap_err();
        assert!(err.contains("already exists") && err.contains("Nothing was changed"), "{}", err);
        state_b(&env); no_leftovers(&env);
        assert!(env.exists("Author A/Book One (1)/something-the-user-made.txt"));
        assert_eq!(names(&env), before, "no safety copy or other file is left in the backups folder");
        assert!(!env.exists("Author B"), "the steps that had already been done were put back");
    }

    #[test]
    fn a_database_that_cannot_be_replaced_changes_nothing() {
        let env = Env::new(); edited(&env);
        fs::create_dir_all(env.path("metadata.db.pandora-old/blocker")).unwrap();   // the swap cannot happen
        let before = names(&env);
        let err = restore(&env.backups, &env.lib, "Calibre Library-1000.db", 3000).unwrap_err();
        assert!(err.contains("could not be replaced"), "{}", err);
        state_b(&env);
        assert!(!env.exists("metadata.db.pandora-restore"));
        assert_eq!(names(&env), before);
    }

    #[test]
    fn another_librarys_backup_is_neither_listed_nor_accepted() {
        let env = Env::new(); edited(&env);
        make_db(&env.backups.join(format!("{}-1500.db", LIB)), "99999999-0000-0000-0000-000000000000", &[(1, "Somewhere/Else (1)")]);
        assert!(list_backups(&env.backups, &env.lib).iter().all(|b| b.taken != 1500));
        let err = restore(&env.backups, &env.lib, "Calibre Library-1500.db", 3000).unwrap_err();
        assert!(err.contains("different library"), "{}", err);
        state_b(&env);
    }

    #[test]
    fn odd_file_names_are_refused() {
        let env = Env::new(); edited(&env);
        for bad in ["../x.db", "..\\x.db", "sub/Calibre Library-1000.db", "Calibre Library-abc.db", "Other Library-1000.db", "Calibre Library-1000.txt", ""] {
            assert!(restore(&env.backups, &env.lib, bad, 3000).is_err(), "{:?} was accepted", bad);
        }
        state_b(&env);
    }

    #[test]
    fn a_damaged_backup_is_not_used() {
        let env = Env::new(); edited(&env);
        fs::write(env.backups.join(format!("{}-1600.db", LIB)), b"this is not a database at all").unwrap();
        assert!(list_backups(&env.backups, &env.lib).iter().all(|b| b.taken != 1600));
        let err = restore(&env.backups, &env.lib, "Calibre Library-1600.db", 3000).unwrap_err();
        assert!(err.contains("damaged"), "{}", err);
        state_b(&env); no_leftovers(&env);
    }

    #[test]
    fn a_log_that_points_outside_the_library_is_refused() {
        let env = Env::new(); edited(&env);
        log(&env, &[(2500, "../outside", "Author A/x"), (2500, "/etc/passwd", "Author A/y")]);
        let err = restore(&env.backups, &env.lib, "Calibre Library-1000.db", 3000).unwrap_err();
        assert!(err.contains("outside the library"), "{}", err);
        state_b(&env);
    }

    #[test]
    fn a_damaged_line_in_the_log_is_skipped() {
        let env = Env::new(); edited(&env);
        log(&env, &[]);
        let mut text = log_text(&env);
        text.push_str("garbage line\n\t\t\nnot-a-number\ta\tb\n2000\tonly-one-field\n");
        fs::write(env.backups.join(format!("{}-renames.log", LIB)), text).unwrap();
        let report = restore(&env.backups, &env.lib, "Calibre Library-1000.db", 3000).unwrap();
        assert_eq!(report.moves_undone, 4);
        state_a(&env);
    }

    #[test]
    fn books_whose_folder_is_missing_are_reported() {
        let env = Env::new(); edited(&env);
        make_db(&env.backups.join(format!("{}-1700.db", LIB)), UUID, &[(1, "Author A/Book One (1)"), (4, "Author D/Gone (4)")]);
        let report = restore(&env.backups, &env.lib, "Calibre Library-1700.db", 3000).unwrap();
        assert_eq!(report.missing_folders, 1);
        assert_eq!(report.missing_examples, vec!["Author D/Gone (4)".to_string()]);
    }

    #[test]
    fn with_no_log_at_all_only_the_database_is_replaced() {
        let env = Env::new();
        make_db(&env.db(), UUID, &[(1, "A/B (1)")]); touch(&env.path("A/B (1)/x.epub"));
        fs::copy(env.db(), env.backups.join(format!("{}-1000.db", LIB))).unwrap();
        make_db(&env.db(), UUID, &[(1, "A/B (1)"), (2, "A/C (2)")]);
        let report = restore(&env.backups, &env.lib, "Calibre Library-1000.db", 2000).unwrap();
        assert_eq!(report.moves_undone, 0);
        assert_eq!(paths(&env.db()), vec![(1, "A/B (1)".to_string())]);
    }

    #[test]
    fn stale_journal_files_do_not_follow_the_restored_database() {
        let env = Env::new(); edited(&env);
        fs::write(env.path("metadata.db-journal"), b"left over").unwrap();
        restore(&env.backups, &env.lib, "Calibre Library-1000.db", 3000).unwrap();
        assert!(!env.exists("metadata.db-journal"));
        state_a(&env);
    }
}
