use dps_local_lib::db::{self, audit, LOCAL_USER_ID};
use rusqlite::Connection;
use dps_local_lib::services::tags;

fn fresh_db() -> Connection { let conn = Connection::open_in_memory().unwrap(); db::run_migrations(&conn).unwrap(); conn }

#[test]
fn migrations_are_versioned_and_idempotent() {
    let conn = fresh_db(); db::run_migrations(&conn).unwrap();
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM _migrations", [], |r| r.get(0)).unwrap(); assert_eq!(count, 1);
    let user: String = conn.query_row("SELECT username FROM users WHERE id=?1", [LOCAL_USER_ID], |r| r.get(0)).unwrap(); assert_eq!(user, "local-archivist");
}

#[test]
fn audit_events_are_append_only() {
    let conn = fresh_db(); let event_id = audit::record_event(&conn, LOCAL_USER_ID, "OBJECT_CREATED", "archive_objects", "object-1", None).unwrap();
    assert!(conn.execute("UPDATE audit_events SET action='changed' WHERE id=?1", [&event_id]).is_err());
    assert!(conn.execute("DELETE FROM audit_events WHERE id=?1", [&event_id]).is_err());
}

#[test]
fn tag_deactivation_preserves_existing_object_links_and_is_audited() {
    let mut conn = fresh_db();
    conn.execute("INSERT INTO archive_objects(id,display_name,original_name,status,document_family_id,created_at,updated_at) VALUES('object-1','Document','Document','Active','object-1','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')", []).unwrap();
    tags::add_tag_to_object(&mut conn, "object-1", "Important").unwrap();
    let tag_id: String = conn.query_row("SELECT id FROM ref_tags WHERE tag_name='important'", [], |r| r.get(0)).unwrap();
    tags::delete_tag(&conn, &tag_id).unwrap();
    let active: i64 = conn.query_row("SELECT is_active FROM ref_tags WHERE id=?1", [&tag_id], |r| r.get(0)).unwrap();
    let links: i64 = conn.query_row("SELECT COUNT(*) FROM map_file_tags WHERE tag_id=?1", [&tag_id], |r| r.get(0)).unwrap();
    let audit_count: i64 = conn.query_row("SELECT COUNT(*) FROM audit_events WHERE action='TAG_DEACTIVATED'", [], |r| r.get(0)).unwrap();
    assert_eq!((active, links, audit_count), (0, 1, 1));
}
