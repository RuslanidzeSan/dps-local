pub mod audit;
pub mod schema_repair;

use rusqlite::{Connection, OptionalExtension};
use std::fs;
use std::path::{Path, PathBuf};

pub const LOCAL_USER_ID: &str = "00000000-0000-4000-8000-000000000001";
const INITIAL_SCHEMA: &str = include_str!("../migrations/001_initial_schema.sql");

pub fn get_database_path() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".dps-local");
    path.push("archive.db");
    path
}

pub fn get_storage_dir() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".dps-local");
    path.push("storage");
    path
}

pub fn get_exports_dir() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".dps-local");
    path.push("exports");
    path
}

pub fn get_thumbnails_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Не вдалося визначити домашню директорію")?;
    let dir = home.join(".dps-local").join("thumbnails");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

pub fn open_database() -> Result<Connection, String> { open_database_at(&get_database_path()) }

pub fn open_database_at(path: &Path) -> Result<Connection, String> {
    if let Some(parent) = path.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    run_migrations(&conn)?;
    schema_repair::ensure_columns(&conn).map_err(|e| e.to_string())?;   // ← саме тут, перед Ok(conn)
    Ok(conn)
}

pub fn run_migrations(conn: &Connection) -> Result<(), String> {
    conn.execute_batch("PRAGMA foreign_keys = ON; CREATE TABLE IF NOT EXISTS _migrations (name TEXT PRIMARY KEY, applied_at TEXT NOT NULL);")
        .map_err(|e| e.to_string())?;
    let applied: Option<String> = conn.query_row("SELECT name FROM _migrations WHERE name = '001_initial_schema'", [], |r| r.get(0))
        .optional().map_err(|e| e.to_string())?;
    if applied.is_none() {
        conn.execute_batch(INITIAL_SCHEMA).map_err(|e| e.to_string())?;
        migrate_legacy_runtime_schema(conn)?;
        conn.execute("INSERT INTO _migrations (name, applied_at) VALUES ('001_initial_schema', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))", [])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Earlier development builds created `archive_files` directly at application
/// startup. Preserve their data while moving them to the versioned model.
fn migrate_legacy_runtime_schema(conn: &Connection) -> Result<(), String> {
    let legacy_exists: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='archive_files')", [], |r| r.get(0)).map_err(|e| e.to_string())?;
    if !legacy_exists { return Ok(()); }
    if !column_exists(conn, "representations", "representation_type")? {
        conn.execute("ALTER TABLE representations ADD COLUMN representation_type TEXT", []).map_err(|e| e.to_string())?;
        conn.execute("UPDATE representations SET representation_type=COALESCE(role,'original')", []).map_err(|e| e.to_string())?;
    }
    if !column_exists(conn, "storage_locations", "storage_type")? {
        conn.execute("ALTER TABLE storage_locations ADD COLUMN storage_type TEXT NOT NULL DEFAULT 'managed'", []).map_err(|e| e.to_string())?;
    }
    if !column_exists(conn, "storage_locations", "is_accessible")? {
        conn.execute("ALTER TABLE storage_locations ADD COLUMN is_accessible INTEGER NOT NULL DEFAULT 1", []).map_err(|e| e.to_string())?;
    }
    conn.execute_batch("INSERT OR IGNORE INTO archive_objects(id,display_name,original_name,description,status,document_family_id,created_at,updated_at) SELECT id,display_name,display_name,description,status,id,created_at,created_at FROM archive_files;")
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool, String> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})")).map_err(|e| e.to_string())?;
    let names = stmt.query_map([], |row| row.get::<_, String>(1)).map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok(names.iter().any(|name| name == column))
}
