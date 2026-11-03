use rusqlite::Connection;
use std::path::PathBuf;

/// Створює тимчасовий каталог для тестової бази.
fn temp_db_path(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dps_migration_v2_test_{}_{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("archive.db")
}

/// Симуляція «СТАРОЇ» бази: schema archive_objects БЕЗ колонки file_extension,
/// з одним реальним записом, який треба зберегти.
fn make_legacy_db(path: &PathBuf) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            username TEXT NOT NULL UNIQUE,
            full_name TEXT NOT NULL,
            role TEXT NOT NULL DEFAULT 'archivist',
            is_active INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS archive_objects (
            record_number INTEGER PRIMARY KEY AUTOINCREMENT,
            id TEXT NOT NULL UNIQUE,
            display_name TEXT NOT NULL,
            original_name TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'Active',
            document_family_id TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
            -- УВАГА: колонки file_extension тут СПЕЦІАЛЬНО немає — це «стара» база
        );
        INSERT INTO archive_objects (id, display_name, original_name, created_at, updated_at)
        VALUES ('11111111-1111-1111-1111-111111111111', 'Стара картка', 'staruy_fayl.pdf',
                '2026-10-01T10:00:00Z', '2026-10-01T10:00:00Z');
        ",
    )
    .unwrap();
}

#[test]
fn repair_adds_missing_columns_and_keeps_data() {
    let path = temp_db_path("repair_adds");
    make_legacy_db(&path);

    // Точка входу програми: open_database_at виконує міграції + (після наших правок) ремонт схеми.
    dps_local_lib::db::open_database_at(&path).expect("open_database_at failed");

    let conn = Connection::open(&path).unwrap();

    // 1) Колонка з'явилась
    let has_column: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('archive_objects') WHERE name = 'file_extension'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(has_column, "file_extension мала бути додана автоматично");

    // 2) Старі дані збережені
    let (display, original): (String, String) = conn
        .query_row(
            "SELECT display_name, original_name FROM archive_objects WHERE id = '11111111-1111-1111-1111-111111111111'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(display, "Стара картка");
    assert_eq!(original, "staruy_fayl.pdf");

    // 3) В старих записах нова колонка = NULL, це нормально
    let ext: Option<String> = conn
        .query_row(
            "SELECT file_extension FROM archive_objects WHERE id = '11111111-1111-1111-1111-111111111111'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(ext.is_none(), "у старих записах file_extension має бути NULL, а не падати");
}

#[test]
fn repair_is_idempotent_running_twice_is_safe() {
    let path = temp_db_path("idempotent");
    make_legacy_db(&path);

    dps_local_lib::db::open_database_at(&path).expect("first run failed");
    dps_local_lib::db::open_database_at(&path).expect("second run failed"); // повторний запуск не ламає базу

    let conn = Connection::open(&path).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM archive_objects", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1, "дублікатів карток бути не повинно");
}

#[test]
fn fresh_database_gets_full_schema() {
    // Нових баз теж не ламає: створення з нуля → повна схема
    let path = temp_db_path("fresh");
    dps_local_lib::db::open_database_at(&path).expect("open_database_at failed");

    let conn = Connection::open(&path).unwrap();
    for table in ["users", "archive_objects", "representations", "storage_locations", "ingest_jobs", "audit_events"] {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |r| r.get(0),
            )
            .unwrap();
        assert!(exists, "таблиця {table} має існувати у новій базі");
    }
}

#[test]
fn audit_guard_trigger_survives_repair() {
    // Страховка: ремонт схеми не повинен знищувати guard-тригер аудиту
    let path = temp_db_path("audit_guard");
    make_legacy_db(&path);
    dps_local_lib::db::open_database_at(&path).expect("open_database_at failed");

    let conn = Connection::open(&path).unwrap();
    conn.execute(
        "INSERT INTO audit_events (id, user_id, action, target_table, target_id, payload, created_at)
         VALUES ('22222222-2222-2222-2222-222222222222', '1', 'TEST', 'x', 'y', '{}', '2026-11-03T00:00:00Z')",
        [],
    )
    .unwrap();
    let result = conn.execute("UPDATE audit_events SET action = 'HACKED' WHERE id = '22222222-2222-2222-2222-222222222222'", []);
    assert!(result.is_err(), "UPDATE по audit_events має бути заблокований guard-тригером");
}
