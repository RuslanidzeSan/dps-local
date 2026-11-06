use rusqlite::{Connection, params};
use dps_local_lib::db;
use dps_local_lib::services::attrs::{self, AttrInfo};

fn fresh_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    db::run_migrations(&conn).unwrap();
    conn
}

fn insert_object(conn: &Connection, id: &str) {
    conn.execute(
        "INSERT INTO archive_objects(id,display_name,original_name,file_extension,description,status,document_family_id,created_at,updated_at) VALUES(?1,'Фото','photo.jpg','.jpg',NULL,'Active',?1,'2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')",
        params![id],
    ).unwrap();
}

fn find<'a>(items: &'a [AttrInfo], name: &str) -> &'a AttrInfo {
    items.iter().find(|a| a.name == name).unwrap_or_else(|| panic!("немає атрибута {name}"))
}

#[test]
fn attr_values_validate_and_persist() {
    let mut conn = fresh_db();
    insert_object(&conn, "o1");

    attrs::add_attr(&mut conn, "Дата зйомки", "date", None, None).unwrap();
    attrs::add_attr(&mut conn, "Об'єктив", "select", None, Some("28mm; 50mm; 85mm")).unwrap();
    attrs::add_attr(&mut conn, "Сторінок", "number", Some("стор."), None).unwrap();

    // коректні значення
    attrs::set_object_attr_value(&conn, "o1", find(&attrs::get_all_attrs_with_stats(&conn).unwrap(), "Дата зйомки").id.as_str(), Some("1974-07-06")).unwrap();
    attrs::set_object_attr_value(&conn, "o1", find(&attrs::get_all_attrs_with_stats(&conn).unwrap(), "Об'єктив").id.as_str(), Some("50mm")).unwrap();
    attrs::set_object_attr_value(&conn, "o1", find(&attrs::get_all_attrs_with_stats(&conn).unwrap(), "Сторінок").id.as_str(), Some("25")).unwrap();

    // повторний запис — UPSERT без дублів
    attrs::set_object_attr_value(&conn, "o1", find(&attrs::get_all_attrs_with_stats(&conn).unwrap(), "Сторінок").id.as_str(), Some("26")).unwrap();

    let vals = attrs::get_object_attr_values(&conn, "o1").unwrap();
    let v = |n: &str| vals.iter().find(|x| x.attr_name == n).unwrap();
    assert_eq!(v("Дата зйомки").value.as_deref(), Some("1974-07-06"));
    assert_eq!(v("Об'єктив").value.as_deref(), Some("50mm"));
    assert_eq!(v("Сторінок").value.as_deref(), Some("26"), "число 26.0 має показуватись без дробової частини");
    assert_eq!(v("Сторінок").unit.as_deref(), Some("стор."));

    // usage_count у довіднику підрос
    let stats = attrs::get_all_attrs_with_stats(&conn).unwrap();
    assert_eq!(find(&stats, "Дата зйомки").usage_count, 1);
    assert_eq!(find(&stats, "Об'єктив").usage_count, 1);
    assert_eq!(find(&stats, "Сторінок").usage_count, 1, "UPSERT не має дублювати зв'язок");
}

#[test]
fn attr_values_reject_bad_input_and_clear() {
    let mut conn = fresh_db();
    insert_object(&conn, "o1");
    attrs::add_attr(&mut conn, "Дата зйомки", "date", None, None).unwrap();
    attrs::add_attr(&mut conn, "Об'єктив", "select", None, Some("28mm; 50mm")).unwrap();
    attrs::add_attr(&mut conn, "Сторінок", "number", Some("стор."), None).unwrap();
    let ids = attrs::get_all_attrs_with_stats(&conn).unwrap();
    let (id_date, id_sel, id_num) = (
        find(&ids, "Дата зйомки").id.clone(),
        find(&ids, "Об'єктив").id.clone(),
        find(&ids, "Сторінок").id.clone(),
    );

    // погані значення відкидаються з поясненням
    assert!(attrs::set_object_attr_value(&conn, "o1", &id_date, Some("1974-13-40")).unwrap_err().contains("Некоректна дата"));
    assert!(attrs::set_object_attr_value(&conn, "o1", &id_num, Some("abc")).unwrap_err().contains("не є числом"));
    assert!(attrs::set_object_attr_value(&conn, "o1", &id_sel, Some("13mm")).unwrap_err().contains("не входить"));
    // неіснуючий об'єкт
    assert!(attrs::set_object_attr_value(&conn, "no-such", &id_date, Some("1974-07-06")).unwrap_err().contains("не знайдено"));

    // збереження і очищення
    attrs::set_object_attr_value(&conn, "o1", &id_date, Some("1974-07-06")).unwrap();
    attrs::set_object_attr_value(&conn, "o1", &id_date, None).unwrap();
    let vals = attrs::get_object_attr_values(&conn, "o1").unwrap();
    assert_eq!(vals.iter().find(|x| x.attr_id == id_date).unwrap().value, None);
}

#[test]
fn attr_values_batch_for_visible_objects() {
    let mut conn = fresh_db();
    insert_object(&conn, "o1");
    insert_object(&conn, "o2");
    attrs::add_attr(&mut conn, "Дата зйомки", "date", None, None).unwrap();
    attrs::add_attr(&mut conn, "Сторінок", "number", Some("стор."), None).unwrap();
    let ids = attrs::get_all_attrs_with_stats(&conn).unwrap();
    let id_date = ids.iter().find(|a| a.name == "Дата зйомки").unwrap().id.clone();
    let id_num = ids.iter().find(|a| a.name == "Сторінок").unwrap().id.clone();

    attrs::set_object_attr_value(&conn, "o1", &id_date, Some("1974-07-06")).unwrap();
    attrs::set_object_attr_value(&conn, "o2", &id_num, Some("25")).unwrap();
    attrs::set_object_attr_value(&conn, "o2", &id_date, Some("2001-09-14")).unwrap();

    // пустий запит — без SQL-помилок (IN () не збираємо)
    assert!(attrs::get_attr_values_for_objects(&conn, &[]).unwrap().is_empty());

    let rows = attrs::get_attr_values_for_objects(&conn, &["o1".into(), "o2".into(), "no-such".into()]).unwrap();
    assert_eq!(rows.len(), 3, "лише заповнені значення, і тільки для запитаних об'єктів");
    let o2: Vec<_> = rows.iter().filter(|r| r.object_id == "o2").collect();
    assert_eq!(o2[0].attr_name, "Дата зйомки", "сортовано за назвою атрибута");
    assert_eq!(o2[1].value, "25", "число без дробової частини");
    assert_eq!(o2[1].unit.as_deref(), Some("стор."));
}

fn insert_object_with_storage(conn: &Connection, id: &str) {
    let now = "2026-01-01T00:00:00Z";
    conn.execute(
        "INSERT INTO archive_objects(id,display_name,original_name,file_extension,description,status,document_family_id,created_at,updated_at) VALUES(?1,?2,?2,'.jpg',NULL,'Active',?1,?3,?3)",
        params![id, format!("file_{id}"), now],
    ).unwrap();
    conn.execute(
        "INSERT INTO representations(id,object_id,representation_type,created_at) VALUES(?1,?2,'original',?3)",
        params![format!("rep_{id}"), id, now],
    ).unwrap();
    conn.execute(
        "INSERT INTO storage_locations(id,representation_id,storage_type,file_path,file_size_bytes,checksum_sha256,is_accessible,created_at) VALUES(?1,?2,'managed','/tmp/f',?3,NULL,1,?4)",
        params![format!("st_{id}"), format!("rep_{id}"), 1024, now],
    ).unwrap();
}

#[test]
fn search_filters_by_attribute_value() {
    use dps_local_lib::services::search::{SearchFilters, search_archive_objects_filtered};
    let mut conn = fresh_db();
    insert_object_with_storage(&conn, "o1");
    insert_object_with_storage(&conn, "o2");
    attrs::add_attr(&mut conn, "Об'єктив", "select", None, Some("28mm; 50mm")).unwrap();
    attrs::add_attr(&mut conn, "Дата зйомки", "date", None, None).unwrap();
    let ids = attrs::get_all_attrs_with_stats(&conn).unwrap();
    let id_lens = ids.iter().find(|a| a.name == "Об'єктив").unwrap().id.clone();
    let id_date = ids.iter().find(|a| a.name == "Дата зйомки").unwrap().id.clone();

    attrs::set_object_attr_value(&conn, "o1", &id_lens, Some("50mm")).unwrap();
    attrs::set_object_attr_value(&conn, "o2", &id_date, Some("1974-07-06")).unwrap();

    let r = search_archive_objects_filtered(&conn, &SearchFilters {
        attr_id: Some(id_lens.clone()), attr_text: Some("50".into()), ..Default::default()
    }).unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].id, "o1");

    let r = search_archive_objects_filtered(&conn, &SearchFilters {
        attr_id: Some(id_date.clone()), attr_text: Some("1974".into()), ..Default::default()
    }).unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].id, "o2");

    assert!(search_archive_objects_filtered(&conn, &SearchFilters {
        attr_id: Some(id_lens.clone()), attr_text: Some("85mm".into()), ..Default::default()
    }).unwrap().is_empty());

    let r = search_archive_objects_filtered(&conn, &SearchFilters {
        attr_id: Some(id_lens), attr_text: None, ..Default::default()
    }).unwrap();
    assert_eq!(r.len(), 2);
}

#[test]
fn export_contains_attribute_columns_and_values() {
    use dps_local_lib::services::export::{export_objects_csv, export_objects_json};
    let mut conn = fresh_db();
    insert_object_with_storage(&conn, "o1");
    insert_object_with_storage(&conn, "o2");
    attrs::add_attr(&mut conn, "Об'єктив", "select", None, Some("28mm; 50mm")).unwrap();
    let ids = attrs::get_all_attrs_with_stats(&conn).unwrap();
    let id_lens = ids.iter().find(|a| a.name == "Об'єктив").unwrap().id.clone();
    attrs::set_object_attr_value(&conn, "o1", &id_lens, Some("50mm")).unwrap();

    let csv = export_objects_csv(&conn).unwrap();
    assert!(csv.contains("checksum_sha256,Об'єктив\n"), "немає колонки атрибута: {csv}");
    assert!(csv.contains("50mm"));
    assert_eq!(csv.lines().count(), 3, "заголовок + 2 об'єкти");

    let json = export_objects_json(&conn).unwrap();
    assert!(json.contains("\"attributes\""));
    assert!(json.contains("\"Об'єктив\": \"50mm\""));
}
