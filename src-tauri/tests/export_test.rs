//! Інтеграційні тести для експорту метаданих (CSV/JSON).
//! Запуск: npm run test:rust

use dps_local_lib::db::open_database_at;
use dps_local_lib::services::ingest::process_ingest;
use dps_local_lib::services::export::{export_objects_csv, export_objects_csv_filtered, export_objects_json};
use std::fs;

fn setup_with_data() -> (tempfile::TempDir, rusqlite::Connection) {
    let temp = tempfile::tempdir().unwrap();
    let storage = temp.path().join("storage");
    let db_path = temp.path().join("archive.db");
    let mut conn = open_database_at(&db_path).unwrap();

    // Створюємо тестові дані
    let source1 = temp.path().join("doc1.txt");
    let source2 = temp.path().join("doc2.pdf");
    fs::write(&source1, b"Document 1").unwrap();
    fs::write(&source2, b"Document 2").unwrap();

    process_ingest(&mut conn, &source1, &storage, "00000000-0000-4000-8000-000000000001").unwrap();
    process_ingest(&mut conn, &source2, &storage, "00000000-0000-4000-8000-000000000001").unwrap();

    (temp, conn)
}

#[test]
fn export_csv_returns_valid_csv_with_header() {
    let (_temp, conn) = setup_with_data();

    let csv = export_objects_csv(&conn).unwrap();

    // Перевіряємо, що є заголовок
    assert!(csv.contains("id,display_name,original_name,file_extension,status,created_at,file_size_bytes,checksum_sha256"));
    
    // Перевіряємо, що є дані
    assert!(csv.contains("doc1.txt"));
    assert!(csv.contains("doc2.pdf"));
    
    // Перевіряємо кількість рядків (заголовок + 2 дані)
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 3);
}

#[test]
fn export_json_returns_valid_json_array() {
    let (_temp, conn) = setup_with_data();

    let json = export_objects_json(&conn).unwrap();

    // Перевіряємо, що це валідний JSON масив
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.is_array());
    
    let arr = parsed.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    
    // Перевіряємо структуру першого об'єкта
    let first = &arr[0];
    assert!(first.get("id").is_some());
    assert!(first.get("display_name").is_some());
    assert!(first.get("checksum_sha256").is_some());
}

#[test]
fn export_respects_filters() {
    let (_temp, conn) = setup_with_data();

    // Експортуємо лише .txt файли
    let csv = export_objects_csv_filtered(&conn, Some("txt")).unwrap();
    
    assert!(csv.contains("doc1.txt"));
    assert!(!csv.contains("doc2.pdf"));
}
