//! Інтеграційні тести: ініціалізація SQLite, міграції, дефолтний користувач,
//! append-only властивість audit_events (Етап 2, ADR-001).
//!
//! Запуск: npm run test:rust   (або cargo test --manifest-path src-tauri/Cargo.toml)
//!
//! УВАГА: тут використовується ім'я крейта `dps_local_lib`. Якщо у вашому
//! src-tauri/Cargo.toml у секції [package] name = "ІНША_НАЗВА" — замініть
//! `dps_local_lib::` на відповідне ім'я (дефіси → підкреслення).

use dps_local_lib::db;
use dps_local_lib::db::audit;
use rusqlite::Connection;

/// Допоміжна функція: свіжа база в пам'яті з усіма міграціями.
fn fresh_db() -> Connection {
    let conn = Connection::open_in_memory().expect("не вдалося створити БД в пам'яті");
    db::run_migrations(&conn).expect("міграції мають застосуватися до нової БД");
    conn
}

/// Тест 1 (tracer bullet): нова БД ініціалізується, міграція застосовується,
/// усі таблиці погодженої моделі існують.
#[test]
fn fresh_database_is_initialized_with_full_schema() {
    let conn = fresh_db();

    let applied: i64 = conn
        .query_row("SELECT COUNT(*) FROM _migrations", [], |r| r.get(0))
        .expect("таблиця _migrations має існувати");
    assert_eq!(applied, 1, "має бути застосовано рівно одну міграцію");

    let migration_name: String = conn
        .query_row("SELECT name FROM _migrations", [], |r| r.get(0))
        .expect("запис про міграцію має існувати");
    assert_eq!(migration_name, "001_initial_schema");

    for table in [
        "users",
        "archive_objects",
        "representations",
        "storage_locations",
        "ingest_jobs",
        "audit_events",
    ] {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |r| r.get(0),
            )
            .unwrap_or_else(|_| panic!("не вдалося перевірити таблицю {table}"));
        assert_eq!(count, 1, "таблиця {table} має існувати");
    }
}

/// Тест 2: повторний запуск міграцій ідемпотентний — база не ламається,
/// міграція не дублюється, дефолтний користувач не множиться.
#[test]
fn migrations_are_idempotent_on_second_run() {
    let conn = fresh_db();

    db::run_migrations(&conn).expect("повторний запуск міграцій має бути безпечним");

    let applied: i64 = conn
        .query_row("SELECT COUNT(*) FROM _migrations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(applied, 1, "міграція не має застосовуватися двічі");

    let users: i64 = conn
        .query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))
        .unwrap();
    assert_eq!(users, 1, "дефолтний користувач не має дублюватися");
}

/// Тест 3: при створенні БД автоматично з'являється системний користувач
/// "Local Archivist" (ADR-001, рішення 5).
#[test]
fn default_local_archivist_user_is_seeded() {
    let conn = fresh_db();

    let (username, full_name, role, is_active): (String, String, String, i64) = conn
        .query_row(
            "SELECT username, full_name, role, is_active FROM users",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .expect("дефолтний користувач має існувати");

    assert_eq!(username, "local-archivist");
    assert_eq!(full_name, "Local Archivist");
    assert_eq!(role, "Archivist");
    assert_eq!(is_active, 1);
}

/// Тест 4: audit_events — append-only. Записати подію можна,
/// підробити (UPDATE) або стерти (DELETE) її — ні.
#[test]
fn audit_events_reject_update_and_delete() {
    let conn = fresh_db();

    let event_id = audit::record_event(
        &conn,
        "00000000-0000-4000-8000-000000000001", // local-archivist
        "OBJECT_CREATED",
        "archive_objects",
        "11111111-1111-4111-8111-111111111111",
        None,
    )
    .expect("запис події в аудит має бути можливим");

    let tampered = conn.execute(
        "UPDATE audit_events SET action = 'TAMPERED' WHERE id = ?1",
        [&event_id],
    );
    assert!(
        tampered.is_err(),
        "UPDATE audit_events має бути заборонено"
    );

    let erased = conn.execute("DELETE FROM audit_events", []);
    assert!(
        erased.is_err(),
        "DELETE audit_events має бути заборонено"
    );

    let action: String = conn
        .query_row(
            "SELECT action FROM audit_events WHERE id = ?1",
            [&event_id],
            |r| r.get(0),
        )
        .expect("запис аудиту має залишитися на місці");
    assert_eq!(action, "OBJECT_CREATED", "запис не має змінитися");
}
