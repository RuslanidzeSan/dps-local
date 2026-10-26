//! Шар керування локальною базою даних: відкриття, міграції.
//!
//! Поклади цей файл у `src-tauri/src/db/mod.rs`.
//! Міграційний SQL — у `src-tauri/migrations/001_initial_schema.sql`.

pub mod audit;

use rusqlite::{Connection};
use std::path::Path;

/// Реєстр міграцій у порядку застосування.
/// include_str! вбудовує вміст SQL-файлу в бінарник під час компіляції —
/// у скомпільованій програмі файли міграцій окремо не потрібні.
const MIGRATIONS: &[(&str, &str)] = &[(
    "001_initial_schema",
    include_str!("../../migrations/001_initial_schema.sql"),
)];

/// Відкрити (або створити) файл бази за шляхом і застосувати всі нові міграції.
pub fn open_database(path: &Path) -> rusqlite::Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(14 /* SQLITE_CANTOPEN */),
                Some(format!("не вдалося створити каталог для БД: {e}")),
            )
        })?;
    }

    let conn = Connection::open(path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    run_migrations(&conn)?;
    Ok(conn)
}

/// Застосувати всі ще не застосовані міграції. Ідемпотентна: повторний
/// виклик не виконує жодної роботи і не ламає існуючу базу.
pub fn run_migrations(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS _migrations (
            name        TEXT PRIMARY KEY,
            applied_at  TEXT NOT NULL
                        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );",
    )?;

    let mut applied = 0usize;

    for (name, sql) in MIGRATIONS {
        let already_applied: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM _migrations WHERE name = ?1)",
            [*name],
            |r| r.get(0),
        )?;

        if already_applied {
            continue;
        }

        // Одна міграція = одна транзакція: або вся схема застосувалася, або нічого.
        let tx = conn.unchecked_transaction()?;
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT INTO _migrations (name) VALUES (?1)",
            [*name],
        )?;
        tx.commit()?;

        applied += 1;
    }

    Ok(applied)
}
