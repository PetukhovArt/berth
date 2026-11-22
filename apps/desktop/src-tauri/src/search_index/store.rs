use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

use rusqlite::{Connection, ErrorCode, OpenFlags};

const SCHEMA_VERSION: i64 = 1;
const BUSY: Duration = Duration::from_secs(5);

const SCHEMA: &str = "
BEGIN;
CREATE TABLE files(
    path TEXT PRIMARY KEY,
    harness TEXT NOT NULL,
    session TEXT NOT NULL,
    size INTEGER NOT NULL,
    mtime INTEGER NOT NULL,
    offset INTEGER NOT NULL
);
CREATE TABLE progress(indexed INTEGER NOT NULL, total INTEGER NOT NULL);
INSERT INTO progress VALUES(0, 0);
CREATE VIRTUAL TABLE messages USING fts5(
    text,
    harness UNINDEXED,
    session UNINDEXED,
    key UNINDEXED,
    role UNINDEXED,
    at UNINDEXED,
    path UNINDEXED,
    tokenize='trigram'
);
PRAGMA user_version=1;
COMMIT;
";

pub fn db_path() -> PathBuf {
    berth_core::paths::search_index_db()
}

pub fn fts5_ready() -> Result<(), String> {
    static READY: OnceLock<Result<(), String>> = OnceLock::new();
    READY
        .get_or_init(|| {
            let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
            conn.execute_batch("CREATE VIRTUAL TABLE probe USING fts5(text, tokenize='trigram')")
                .map_err(|e| {
                    format!("the bundled SQLite lacks FTS5 with the trigram tokenizer: {e}")
                })
        })
        .clone()
}

pub fn is_corrupt(error: &rusqlite::Error) -> bool {
    matches!(
        error.sqlite_error_code(),
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase)
    )
}

pub fn remove_files() -> io::Result<()> {
    let path = db_path();
    for suffix in ["", "-wal", "-shm"] {
        let mut name = path.clone().into_os_string();
        name.push(suffix);
        match fs::remove_file(PathBuf::from(name)) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

fn version(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
}

fn prepare() -> rusqlite::Result<Option<Connection>> {
    let path = db_path();
    if let Some(dir) = path.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let conn = Connection::open(&path)?;
    conn.busy_timeout(BUSY)?;
    conn.query_row("PRAGMA journal_mode=WAL", [], |row| row.get::<_, String>(0))?;
    conn.execute_batch("PRAGMA synchronous=NORMAL")?;
    let found = version(&conn)?;
    if found == SCHEMA_VERSION {
        return Ok(Some(conn));
    }
    let tables: i64 = conn.query_row("SELECT count(*) FROM sqlite_master", [], |row| row.get(0))?;
    if found == 0 && tables == 0 {
        conn.execute_batch(SCHEMA)?;
        return Ok(Some(conn));
    }
    Ok(None)
}

pub fn open_writer() -> Result<Connection, String> {
    fts5_ready()?;
    match prepare() {
        Ok(Some(conn)) => return Ok(conn),
        Ok(None) => {}
        Err(e) if is_corrupt(&e) => {}
        Err(e) => return Err(e.to_string()),
    }
    remove_files().map_err(|e| format!("{}: {e}", db_path().display()))?;
    match prepare() {
        Ok(Some(conn)) => Ok(conn),
        Ok(None) => Err("search index: schema did not initialise".to_owned()),
        Err(e) => Err(e.to_string()),
    }
}

pub fn open_reader() -> Result<Option<Connection>, String> {
    fts5_ready()?;
    let path = db_path();
    if !path.is_file() {
        return Ok(None);
    }
    let Ok(conn) = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY) else {
        return Ok(None);
    };
    conn.busy_timeout(BUSY).map_err(|e| e.to_string())?;
    match version(&conn) {
        Ok(SCHEMA_VERSION) => Ok(Some(conn)),
        _ => Ok(None),
    }
}
