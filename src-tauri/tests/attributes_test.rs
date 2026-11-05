use rusqlite::Connection;
use dps_local_lib::db;
use dps_local_lib::services::attrs;

fn temp_db_path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("dps_attrs_test_{}_{}", name, std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("archive.db")
}

#[test]
fn attr_crud_full_cycle() {
    let path = temp_db_path("crud");
    let mut conn = Connection::open(&path).unwrap();
    db::run_migrations(&conn).unwrap();

    attrs::add_attr(&mut conn, "Дата зйомки", "date", None, None).unwrap();
    attrs::add_attr(&mut conn, "Об'єктив", "select", None, Some("28mm; 50mm;85mm")).unwrap();
    // Дублікат назви (UNIQUE NOCASE) — помилка
    assert!(attrs::add_attr(&mut conn, "дата зйомки", "text", None, None).is_err());
    // Невірний тип — помилка
    assert!(attrs::add_attr(&mut conn, "Щось", "color", None, None).is_err());
    // select без значень — помилка
    assert!(attrs::add_attr(&mut conn, "Колір", "select", None, None).is_err());

    let all = attrs::get_all_attrs_with_stats(&conn).unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[0].code, "дата_зйомки");
    let lens = all.iter().find(|a| a.name == "Об'єктив").unwrap();
    assert_eq!(lens.list_values.as_deref(), Some("28mm;50mm;85mm"));
    assert_eq!(lens.usage_count, 0);

    attrs::update_attr(&conn, &lens.id, "Об'єктив фотокамери").unwrap();
    let all = attrs::get_all_attrs_with_stats(&conn).unwrap();
    assert!(all.iter().any(|a| a.name == "Об'єктив фотокамери"));

    attrs::delete_attr(&conn, &lens.id).unwrap();
    let all = attrs::get_all_attrs_with_stats(&conn).unwrap();
    assert_eq!(all.len(), 1);

    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}

#[test]
fn attr_migration_is_idempotent() {
    let path = temp_db_path("idem");
    let conn = Connection::open(&path).unwrap();
    db::run_migrations(&conn).unwrap();
    db::run_migrations(&conn).unwrap();
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('attr_definitions','attr_values')",
        [], |r| r.get(0)).unwrap();
    assert_eq!(n, 2);
    std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
}
