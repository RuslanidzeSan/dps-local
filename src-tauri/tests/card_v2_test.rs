use dps_local_lib::db;
use dps_local_lib::services::card::{get_document_card, update_document_card, CardUpdate};
use dps_local_lib::services::search::{search_archive_objects_filtered, SearchFilters};
use rusqlite::{params, Connection};
use tempfile::TempDir;

fn setup() -> (TempDir, Connection) {
    let tmp = TempDir::new().unwrap();
    let conn = db::open_database_at(&tmp.path().join("archive.db")).unwrap();
    (tmp, conn)
}

fn insert_object(conn: &Connection, name: &str) -> String {
    let id = format!("obj-{name}");
    let created_at = "2026-01-10T10:00:00+00:00";
    conn.execute(
        "INSERT INTO archive_objects(id,display_name,original_name,status,document_family_id,created_at,updated_at) VALUES(?1,?2,?2,'Active',?1,?3,?3)",
        params![id, name, created_at],
    ).unwrap();
    // Пошук JOIN-ить representations і storage_locations —
    // тому документ мусить мати принаймні один рядок у кожній.
    conn.execute(
        "INSERT INTO representations(id,object_id,representation_type,created_at) VALUES(?1,?2,'original',?3)",
        params![format!("rep-{name}"), id, created_at],
    ).unwrap();
    conn.execute(
        "INSERT INTO storage_locations(id,representation_id,storage_type,file_path,file_size_bytes,created_at) VALUES(?1,?2,'managed',?3,100,?4)",
        params![format!("loc-{name}"), format!("rep-{name}"), format!("/tmp/{name}"), created_at],
    ).unwrap();
    id
}

#[test]
fn repair_adds_card_v2_columns() {
    let (_tmp, conn) = setup();
    for col in ["file_kind", "author", "purpose", "doc_date_from", "doc_date_to", "source_location"] {
        let exists: bool = conn.query_row(
            "SELECT COUNT(*)>0 FROM pragma_table_info('archive_objects') WHERE name=?1",
            [col], |r| r.get(0),
        ).unwrap();
        assert!(exists, "колонки {col} немає після repair");
    }
}

#[test]
fn update_card_persists_and_audits() {
    let (_tmp, conn) = setup();
    let id = insert_object(&conn, "photo");

    update_document_card(&conn, &CardUpdate {
        object_id: id.clone(),
        file_kind: Some("Цифрове фото".into()),
        author: Some("Оператор Руслан".into()),
        purpose: Some("Діджиталізація фонду".into()),
        doc_date_from: Some("1943-01-15".into()),
        doc_date_to: Some("1945-12-31".into()),
        source_location: Some("Disc_7/Scan_2005/".into()),
        card_template: None,
    }).unwrap();

    let card = get_document_card(&conn, &id).unwrap();
    assert_eq!(card.file_kind.as_deref(), Some("Цифрове фото"));
    assert_eq!(card.author.as_deref(), Some("Оператор Руслан"));
    assert_eq!(card.doc_date_from.as_deref(), Some("1943-01-15"));

    let audit: i64 = conn.query_row(
        "SELECT COUNT(*) FROM audit_events WHERE action='CARD_UPDATED' AND target_id=?1",
        [&id], |r| r.get(0),
    ).unwrap();
    assert_eq!(audit, 1, "оновлення мусить писати подію в аудит");
}

#[test]
fn update_card_rejects_bad_dates_and_unknown_ids() {
    let (_tmp, conn) = setup();
    let id = insert_object(&conn, "doc");

    // Некоректний формат дати
    let bad = CardUpdate { object_id: id.clone(), doc_date_from: Some("31.12.2005".into()), ..Default::default() };
    assert!(update_document_card(&conn, &bad).is_err(), "дата не в YYYY-MM-DD мусить відхилятися");

    // Несуществующий документ
    let ghost = CardUpdate { object_id: "obj-nope".into(), author: Some("хтось".into()), ..Default::default() };
    assert!(update_document_card(&conn, &ghost).is_err());

    // Порожні значення — легальні: очищають поля
    update_document_card(&conn, &CardUpdate { object_id: id, ..Default::default() }).unwrap();
}

#[test]
fn doc_date_filter_matches_by_overlap() {
    let (_tmp, conn) = setup();
    // Один документ з діапазоном дат, один з точною датою, один без дат
    let war = insert_object(&conn, "war-photo");
    insert_object(&conn, "digital");
    insert_object(&conn, "undated");
    conn.execute(
        "UPDATE archive_objects SET doc_date_from='1943-01-01', doc_date_to='1945-12-31' WHERE id=?1",
        [&war],
    ).unwrap();
    conn.execute(
        "UPDATE archive_objects SET doc_date_from='2001-05-01', doc_date_to='2001-05-01' WHERE display_name='digital'",
        [],
    ).unwrap();

    // Запит 1940–1950 → лише воєнне фото (діапазони перетинаються)
    let r = search_archive_objects_filtered(&conn, &SearchFilters {
        doc_date_from: Some("1940-01-01".into()),
        doc_date_to: Some("1950-12-31".into()),
        ..Default::default()
    }).unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].display_name, "war-photo");

    // Ліва межа 1900 → обидва датовані документи (документи без дати не потрапляють)
    let r2 = search_archive_objects_filtered(&conn, &SearchFilters {
        doc_date_from: Some("1900-01-01".into()),
        ..Default::default()
    }).unwrap();
    assert_eq!(r2.len(), 2, "датовані документи: {:?}", r2.iter().map(|o| &o.display_name).collect::<Vec<_>>());
}
