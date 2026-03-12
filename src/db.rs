use rusqlite::{params, Connection, Result};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Entry {
    pub id: i64,
    pub app: String,
    pub title: String,
    pub text: String,
    pub timestamp: i64,
    pub embedding: Vec<u8>,
}

pub fn open_connection(db_path: &Path) -> Result<Connection> {
    let conn = Connection::open(db_path)?;
    // Enable WAL mode for better concurrency (readers don't block writers)
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    Ok(conn)
}

pub fn create_db(db_path: &Path) -> Result<()> {
    let conn = open_connection(db_path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS entries (
             id INTEGER PRIMARY KEY AUTOINCREMENT,
             app TEXT,
             title TEXT,
             text TEXT,
             timestamp INTEGER UNIQUE,
             embedding BLOB
         )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_timestamp ON entries (timestamp)",
        [],
    )?;
    Ok(())
}

pub fn get_all_entries_conn(conn: &Connection) -> Result<Vec<Entry>> {
    let mut stmt = conn.prepare("SELECT id, app, title, text, timestamp, embedding FROM entries ORDER BY timestamp DESC")?;

    let entries_iter = stmt.query_map([], |row| {
        Ok(Entry {
            id: row.get(0)?,
            app: row.get(1)?,
            title: row.get(2)?,
            text: row.get(3)?,
            timestamp: row.get(4)?,
            embedding: row.get(5)?,
        })
    })?;

    let mut entries = Vec::new();
    for entry in entries_iter {
        entries.push(entry?);
    }
    Ok(entries)
}

pub fn get_all_entries(db_path: &Path) -> Result<Vec<Entry>> {
    let conn = open_connection(db_path)?;
    get_all_entries_conn(&conn)
}

pub fn get_timestamps_conn(conn: &Connection) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare("SELECT timestamp FROM entries ORDER BY timestamp DESC")?;

    let timestamps_iter = stmt.query_map([], |row| row.get(0))?;

    let mut timestamps = Vec::new();
    for ts in timestamps_iter {
        timestamps.push(ts?);
    }
    Ok(timestamps)
}

pub fn get_timestamps(db_path: &Path) -> Result<Vec<i64>> {
    let conn = open_connection(db_path)?;
    get_timestamps_conn(&conn)
}

pub fn insert_entry(
    db_path: &Path,
    text: &str,
    timestamp: i64,
    embedding: &[u8],
    app: &str,
    title: &str,
) -> Result<Option<i64>> {
    let conn = open_connection(db_path)?;
    insert_entry_conn(&conn, text, timestamp, embedding, app, title)
}

pub fn insert_entry_conn(
    conn: &Connection,
    text: &str,
    timestamp: i64,
    embedding: &[u8],
    app: &str,
    title: &str,
) -> Result<Option<i64>> {
    let mut stmt = conn.prepare(
        "INSERT INTO entries (text, timestamp, embedding, app, title)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(timestamp) DO NOTHING"
    )?;

    let inserted = stmt.execute(params![text, timestamp, embedding, app, title])?;

    if inserted > 0 {
        Ok(Some(conn.last_insert_rowid()))
    } else {
        Ok(None)
    }
}
