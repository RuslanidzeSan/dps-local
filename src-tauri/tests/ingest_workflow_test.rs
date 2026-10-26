use dps_local_lib::db::open_database;
use dps_local_lib::services::ingest::process_ingest;
use std::fs;

#[test]
fn test_managed_ingest_workflow_success() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("test_archive.db");
    let storage_dir = temp_dir.path().join("storage").join("managed");
    
    // 1. Ініціалізація БД
    let mut conn = open_database(&db_path).unwrap();
    
    // 2. Створюємо гарантованого користувача в БД для Foreign Key
    let user_id = "00000000-0000-0000-0000-000000000001";
    conn.execute(
        "INSERT OR IGNORE INTO users (id, username, full_name, role, is_active, created_at) 
         VALUES (?1, 'test_archivist', 'Test Archivist', 'Archivist', 1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
        [user_id],
    ).unwrap();

    // 3. Створення тестового зовнішнього файлу
    let source_path = temp_dir.path().join("test_document.txt");
    fs::write(&source_path, b"Hello, Archive!").unwrap();
    
    // 4. Запуск Ingest
    let result = process_ingest(&mut conn, &source_path, &storage_dir, user_id);
    
    assert!(result.is_ok(), "Ingest failed: {:?}", result.err());
    let object_id = result.unwrap();

    // 5. Перевірки (Asserts)
    let copied_files: Vec<_> = fs::read_dir(&storage_dir).unwrap().collect();
    assert_eq!(copied_files.len(), 1, "Має бути 1 скопійований файл");
    
    let obj_count: i64 = conn.query_row("SELECT count(*) FROM archive_objects WHERE id = ?1", [&object_id], |r| r.get(0)).unwrap();
    assert_eq!(obj_count, 1, "Archive object should exist");

    let audit_count: i64 = conn.query_row("SELECT count(*) FROM audit_events WHERE action = 'INGEST_COMPLETED'", [], |r| r.get(0)).unwrap();
    assert_eq!(audit_count, 1, "Audit event should exist");
}
