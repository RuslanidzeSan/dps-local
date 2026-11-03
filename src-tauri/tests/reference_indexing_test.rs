//! Інтеграційні тести Reference-only indexing (Етап 5 за планом Brief).
//! Запуск: npm run test:rust

use dps_local_lib::db::open_database_at;
use dps_local_lib::services::ingest::{index_reference, reconcile_reference_locations};
use std::fs;

fn setup() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf, rusqlite::Connection) {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("external.pdf");
    let storage = temp.path().join("storage"); // кероване сховище
    let db_path = temp.path().join("archive.db");
    fs::write(&source, b"external content").unwrap();
    let conn = open_database_at(&db_path).unwrap();
    (temp, source, storage, conn)
}

/// Reference-індексація: картка створюється, файл НЕ копіюється в сховище,
/// оригінал лишається на місці, записується подія аудиту.
#[test]
fn reference_indexing_registers_card_without_copying() {
    let (_temp, source, storage, mut conn) = setup();

    let result = index_reference(&mut conn, &source, "00000000-0000-4000-8000-000000000001").unwrap();

    // Картка існує з storage_type='reference'
    let storage_type: String = conn
        .query_row(
            "SELECT s.storage_type FROM storage_locations s
             JOIN representations r ON r.id = s.representation_id
             WHERE r.object_id = ?1",
            [&result.object_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(storage_type, "reference");

    // Оригінальний файл не змінений
    assert_eq!(fs::read(&source).unwrap(), b"external content");

    // У керованому сховищі НЕ з'явилося жодної копії
    let copied = fs::read_dir(&storage).map(|d| d.count()).unwrap_or(0);
    assert_eq!(copied, 0, "reference-режим не має копіювати файл");

    // Подія аудиту зафіксована
    let audit: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM audit_events WHERE action='REFERENCE_INDEXED' AND target_id=?1",
            [&result.object_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(audit, 1);
}

/// Reconcile: якщо зовнішній файл зник — is_accessible стає 0 і це аудіюється;
/// якщо файл повернувся — is_accessible знову 1.
#[test]
fn reconcile_updates_availability_and_audits_changes() {
    let (_temp, source, _storage, mut conn) = setup();
    let user = "00000000-0000-4000-8000-000000000001";

    let result = index_reference(&mut conn, &source, user).unwrap();

    // Стан до: все доступно, змін немає
    let changed = reconcile_reference_locations(&mut conn).unwrap();
    assert_eq!(changed, 0, "нічого не змінилося — reconcile мовчить");

    // Файл зникає
    fs::remove_file(&source).unwrap();
    let changed = reconcile_reference_locations(&mut conn).unwrap();
    assert_eq!(changed, 1);

    let accessible: i64 = conn
        .query_row(
            "SELECT s.is_accessible FROM storage_locations s
             JOIN representations r ON r.id = s.representation_id
             WHERE r.object_id = ?1",
            [&result.object_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(accessible, 0, "відсутній файл має позначатися як недоступний");

    let audit: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM audit_events WHERE action='REFERENCE_AVAILABILITY_CHANGED'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(audit, 1);

    // Файл повертається
    fs::write(&source, b"external content").unwrap();
    let changed = reconcile_reference_locations(&mut conn).unwrap();
    assert_eq!(changed, 1);

    let accessible: i64 = conn
        .query_row(
            "SELECT s.is_accessible FROM storage_locations s
             JOIN representations r ON r.id = s.representation_id
             WHERE r.object_id = ?1",
            [&result.object_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(accessible, 1, "повернений файл знову доступний");
}
