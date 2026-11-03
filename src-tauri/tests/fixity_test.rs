use dps_local_lib::db;
use dps_local_lib::services::fixity::{verify_archive_fixity, FixityReport};
use dps_local_lib::services::ingest::sha256_file;
use rusqlite::{params, Connection};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn setup() -> (TempDir, Connection) {
    let tmp = TempDir::new().unwrap();
    let conn = db::open_database_at(&tmp.path().join("archive.db")).unwrap();
    (tmp, conn)
}

/// Створює файл у tmp і реєструє його як managed-об'єкт із коректною сумою.
fn insert_managed(conn: &Connection, tmp: &Path, name: &str, contents: &str) -> String {
    let file_path = tmp.join(name);
    fs::write(&file_path, contents).unwrap();
    let checksum = sha256_file(&file_path).unwrap();
    let object_id = format!("obj-{name}");
    let created_at = "2026-01-10T10:00:00+00:00";
    conn.execute(
        "INSERT INTO archive_objects(id,display_name,original_name,status,document_family_id,created_at,updated_at) VALUES(?1,?2,?2,'Active',?1,?3,?3)",
        params![object_id, name, created_at],
    ).unwrap();
    conn.execute(
        "INSERT INTO representations(id,object_id,representation_type,created_at) VALUES(?1,?2,'original',?3)",
        params![format!("rep-{name}"), object_id, created_at],
    ).unwrap();
    conn.execute(
        "INSERT INTO storage_locations(id,representation_id,storage_type,file_path,file_size_bytes,checksum_sha256,created_at) VALUES(?1,?2,'managed',?3,?4,?5,?6)",
        params![format!("loc-{name}"), format!("rep-{name}"), file_path.to_string_lossy(), contents.len() as i64, checksum, created_at],
    ).unwrap();
    object_id
}

#[test]
fn fixity_reports_ok_mismatch_and_missing() {
    let (tmp, conn) = setup();
    // 1) цілий файл, 2) файл, який змінять після реєстрації, 3) файл, який видалять
    insert_managed(&conn, tmp.path(), "good.txt", "AAA");
    insert_managed(&conn, tmp.path(), "bad.txt", "BBB");
    insert_managed(&conn, tmp.path(), "gone.txt", "CCC");

    // Пошкоджуємо bad.txt і видаляємо gone.txt
    fs::write(tmp.path().join("bad.txt"), "BBB-ZZZ").unwrap();
    fs::remove_file(tmp.path().join("gone.txt")).unwrap();

    let report = verify_archive_fixity(&conn).unwrap();

    assert_eq!(report.checked, 3, "перевірено всі три managed-файли");
    assert_eq!(report.ok, 1, "лише good.txt цілий");
    assert_eq!(report.problems.len(), 2);
    let statuses: Vec<&str> = report.problems.iter().map(|p| p.status.as_str()).collect();
    assert!(statuses.contains(&"mismatch"), "bad.txt мусить бути mismatch: {:?}", statuses);
    assert!(statuses.contains(&"missing"), "gone.txt мусить бути missing: {:?}", statuses);
    for p in &report.problems {
        assert!(!p.display_name.is_empty(), "у проблемі має бути назва документа");
    }
}

#[test]
fn fixity_skips_reference_locations_without_checksum() {
    let (tmp, conn) = setup();
    // reference-файл: контрольної суми немає (NULL) — його не перевіряємо
    let created_at = "2026-01-10T10:00:00+00:00";
    conn.execute(
        "INSERT INTO archive_objects(id,display_name,original_name,status,document_family_id,created_at,updated_at) VALUES('obj-ref','ref.txt','ref.txt','Active','obj-ref',?1,?1)",
        params![created_at],
    ).unwrap();
    conn.execute(
        "INSERT INTO representations(id,object_id,representation_type,created_at) VALUES('rep-ref','obj-ref','original',?1)",
        params![created_at],
    ).unwrap();
    conn.execute(
        "INSERT INTO storage_locations(id,representation_id,storage_type,file_path,file_size_bytes,checksum_sha256,is_accessible,created_at) VALUES('loc-ref','rep-ref','reference',?1,10,NULL,1,?2)",
        params![tmp.path().join("ref.txt").to_string_lossy(), created_at],
    ).unwrap();

    let report = verify_archive_fixity(&conn).unwrap();

    assert_eq!(report.checked, 0, "reference не має потрапляти у перевірку");
    assert!(report.problems.is_empty());
}

#[test]
fn fixity_writes_audit_summary_event() {
    let (tmp, conn) = setup();
    insert_managed(&conn, tmp.path(), "good.txt", "AAA");

    let report: FixityReport = verify_archive_fixity(&conn).unwrap();
    assert_eq!(report.ok, 1);

    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM audit_events WHERE action='FIXITY_AUDIT_COMPLETED'",
        [], |r| r.get(0),
    ).unwrap();
    assert_eq!(count, 1, "перевірка мусить лишити підсумковий запис в аудиті");
}
