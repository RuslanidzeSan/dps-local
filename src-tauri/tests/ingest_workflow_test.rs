use dps_local_lib::db::open_database_at;
use dps_local_lib::services::ingest::process_ingest;
use std::fs;

#[test]
fn managed_ingest_creates_card_copy_and_audit_event() {
    let temp = tempfile::tempdir().unwrap(); let source = temp.path().join("document.txt"); let storage = temp.path().join("storage"); let db_path = temp.path().join("archive.db");
    fs::write(&source, b"Hello, Archive!").unwrap(); let mut conn = open_database_at(&db_path).unwrap();
    let result = process_ingest(&mut conn, &source, &storage, "00000000-0000-4000-8000-000000000001").unwrap();
    let objects: i64 = conn.query_row("SELECT COUNT(*) FROM archive_objects WHERE id=?1", [&result.object_id], |r| r.get(0)).unwrap();
    let copies: i64 = conn.query_row("SELECT COUNT(*) FROM storage_locations", [], |r| r.get(0)).unwrap();
    let audit: i64 = conn.query_row("SELECT COUNT(*) FROM audit_events WHERE action='INGEST_COMPLETED'", [], |r| r.get(0)).unwrap();
    assert_eq!((objects, copies, audit), (1, 1, 1)); assert_eq!(fs::read(&source).unwrap(), b"Hello, Archive!");
}
