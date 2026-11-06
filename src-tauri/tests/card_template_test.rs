use rusqlite::{Connection, params};
use dps_local_lib::db;
use dps_local_lib::db::schema_repair;
use dps_local_lib::services::card::{self, CardUpdate};

fn fresh_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::run_migrations(&conn).unwrap();
    schema_repair::ensure_columns(&conn).unwrap();
    conn
}

fn insert_object(conn: &Connection, id: &str) {
    conn.execute(
        "INSERT INTO archive_objects(id,display_name,original_name,file_extension,description,status,document_family_id,created_at,updated_at) VALUES(?1,'Фото','photo.jpg','.jpg',NULL,'Active',?1,'2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
        params![id],
    ).unwrap();
}

fn upd(id: &str, template: Option<&str>) -> CardUpdate {
    CardUpdate {
        object_id: id.to_string(),
        file_kind: None, author: None, purpose: None,
        doc_date_from: None, doc_date_to: None, source_location: None,
        card_template: template.map(|s| s.to_string()),
    }
}

#[test]
fn card_template_persists_validates_and_clears() {
    let conn = fresh_db();
    insert_object(&conn, "o1");

    // шаблон зберігається і читається
    card::update_document_card(&conn, &upd("o1", Some("photo"))).unwrap();
    let c = card::get_document_card(&conn, "o1").unwrap();
    assert_eq!(c.card_template.as_deref(), Some("photo"));

    // невідомий шаблон відкидається з поясненням і не псує збережене
    let err = card::update_document_card(&conn, &upd("o1", Some("bogus"))).unwrap_err();
    assert!(err.contains("Невідомий шаблон"), "неточна помилка: {err}");
    let c = card::get_document_card(&conn, "o1").unwrap();
    assert_eq!(c.card_template.as_deref(), Some("photo"));

    // None очищає шаблон (семантика решти полів картки)
    card::update_document_card(&conn, &upd("o1", None)).unwrap();
    let c = card::get_document_card(&conn, "o1").unwrap();
    assert_eq!(c.card_template, None);
}

#[test]
fn fresh_database_has_card_template_column() {
    let conn = fresh_db();
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('archive_objects') WHERE name='card_template'",
        [], |r| r.get(0)).unwrap();
    assert_eq!(n, 1, "колонки card_template немає після міграцій");
}
