use rusqlite::Connection;

/// Список колонок, які гарантовано мають існувати у таблицях.
/// ПРАВИЛО: щоразу, коли додаєш колонку в міграцію — додай її і сюди.
/// Формат: (таблиця, колонка, SQL-оголошення типу).
pub const DESIRED_COLUMNS: &[(&str, &str, &str)] = &[
    // --- archive_objects ---
    ("archive_objects", "file_extension", "TEXT"),
    // Коли з'являться нові колонки — додавай їх нижче, наприклад:
    // ("archive_objects", "description", "TEXT"),
    // ("archive_objects", "mime_type", "TEXT"),
        // --- archive_objects: архівна картка v2 ---
    ("archive_objects", "file_kind", "TEXT"),
    ("archive_objects", "author", "TEXT"),
    ("archive_objects", "purpose", "TEXT"),
    ("archive_objects", "doc_date_from", "TEXT"),
    ("archive_objects", "doc_date_to", "TEXT"),
    ("archive_objects", "source_location", "TEXT"),
    // --- archive_objects: шаблон архівної картки (A2) ---
    ("archive_objects", "card_template", "TEXT"),
];

/// Додає відсутні колонки до існуючих таблиць. Безпечно викликати повторно.
/// Таблиці, яких ще не існує, пропускає (їх створить міграція).
pub fn ensure_columns(conn: &Connection) -> rusqlite::Result<()> {
    for (table, column, decl) in DESIRED_COLUMNS {
        let table_exists: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |r| r.get(0),
        )?;
        if !table_exists {
            continue; // таблицю створить звичайна міграція
        }
        let column_exists: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info(?1) WHERE name = ?2",
            [table, column],
            |r| r.get(0),
        )?;
        if !column_exists {
            // ALTER TABLE ... ADD COLUMN не може бути параметризованим, тому format!
            // Значення приходять лише з нашого константного списку, не від користувача.
            conn.execute(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"), [])?;
        }
    }
    Ok(())
}
