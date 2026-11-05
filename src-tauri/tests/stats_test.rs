use rusqlite::Connection;
use dps_local_lib::db;
use dps_local_lib::services::stats;

fn temp_db_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("dps_stats_test_{}_{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("archive.db")
}

fn insert_object_with_location(conn: &Connection, oid: &str, status: &str, storage: &str, size: i64, accessible: bool) {
    let now = "2026-01-01T00:00:00Z";
    conn.execute(
        "INSERT INTO archive_objects(id,display_name,original_name,file_extension,description,status,document_family_id,created_at,updated_at) VALUES(?1,?2,?2,'.pdf',NULL,?3,?1,?4,?4)",
        rusqlite::params![oid, format!("file_{}", oid), status, now],
    ).unwrap();
    let rep_id = format!("rep_{}", oid);
    conn.execute(
        "INSERT INTO representations(id,object_id,representation_type,created_at) VALUES(?1,?2,'original',?3)",
        rusqlite::params![rep_id, oid, now],
    ).unwrap();
    conn.execute(
        "INSERT INTO storage_locations(id,representation_id,storage_type,file_path,file_size_bytes,checksum_sha256,is_accessible,created_at) VALUES(?1,?2,?3,'/tmp/f',?4,NULL,?5,?6)",
        rusqlite::params![format!("st_{}", oid), rep_id, storage, size, accessible as i64, now],
    ).unwrap();
}

#[test]
fn registry_stats_counts_statuses_sizes_and_availability() {
    let path = temp_db_path("main");
    let conn = Connection::open(&path).unwrap();
    db::run_migrations(&conn).unwrap();

    insert_object_with_location(&conn, "o1", "Active", "managed", 100, true);
    insert_object_with_location(&conn, "o2", "Active", "reference", 50, false);
    insert_object_with_location(&conn, "o3", "Pending_Deletion", "managed", 200, true);
    insert_object_with_location(&conn, "o4", "Archived", "managed", 10, true);

    let s = stats::get_registry_stats(&conn).unwrap();
    assert_eq!(s.total_objects, 4);
    assert_eq!(s.active, 2);
    assert_eq!(s.pending_deletion, 1);
    assert_eq!(s.checksum_error, 0);
    assert_eq!(s.other_status, 1);
    assert_eq!(s.total_bytes, 360);
    assert_eq!(s.inaccessible_locations, 1);

    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn registry_stats_on_empty_database_is_zeroes() {
    let path = temp_db_path("empty");
    let conn = Connection::open(&path).unwrap();
    db::run_migrations(&conn).unwrap();
    let s = stats::get_registry_stats(&conn).unwrap();
    assert_eq!(s.total_objects, 0);
    assert_eq!(s.active, 0);
    assert_eq!(s.pending_deletion, 0);
    assert_eq!(s.total_bytes, 0);
    assert_eq!(s.inaccessible_locations, 0);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
