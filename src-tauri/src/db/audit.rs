//! Єдиний інтерфейс доступу до audit_events: тільки додавання.
//! Замовчуванням тут НЕМАЄ жодних функцій оновлення чи видалення —
//! це й є гарантія append-only на рівні коду (ADR-001, рішення 4),
//! а guard-тригер у схемі страхує базу на випадок прямого SQL.

use chrono::Utc;
use rusqlite::Connection;
use uuid::Uuid;

/// Зафіксувати подію в аудит-лозі. Повертає UUID створеного запису.
pub fn record_event(
    conn: &Connection,
    user_id: &str,
    action: &str,
    target_table: &str,
    target_id: &str,
    payload: Option<&str>,
) -> rusqlite::Result<String> {
    let id = Uuid::new_v4().to_string();
    let created_at = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

    conn.execute(
        "INSERT INTO audit_events (id, user_id, action, target_table, target_id, payload, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        rusqlite::params![id, user_id, action, target_table, target_id, payload, created_at],
    )?;

    Ok(id)
}
