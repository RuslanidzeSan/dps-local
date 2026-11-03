use dps_local_lib::db;
use dps_local_lib::services::ingest::ArchiveObjectView;
use dps_local_lib::services::search::{search_archive_objects_filtered, SearchFilters};
use rusqlite::{params, Connection};
use tempfile::TempDir;

/// Створює тимчасову базу і вставляє один об'єкт з заданими
/// розширенням, датою створення та розміром файлу.
fn insert_object(conn: &Connection, name: &str, ext: Option<&str>, created_at: &str, size: i64) -> String {
    let object_id = format!("obj-{name}");
    conn.execute(
        "INSERT INTO archive_objects(id,display_name,original_name,file_extension,status,document_family_id,created_at,updated_at) VALUES(?1,?2,?2,?3,'Active',?1,?4,?4)",
        params![object_id, name, ext, created_at],
    ).unwrap();
    let representation_id = format!("rep-{name}");
    conn.execute(
        "INSERT INTO representations(id,object_id,representation_type,created_at) VALUES(?1,?2,'original',?3)",
        params![representation_id, object_id, created_at],
    ).unwrap();
    conn.execute(
        "INSERT INTO storage_locations(id,representation_id,storage_type,file_path,file_size_bytes,checksum_sha256,is_accessible,created_at) VALUES(?1,?2,'managed','/tmp/fake',?3,'abc',1,?4)",
        params![format!("loc-{name}"), representation_id, size, created_at],
    ).unwrap();
    object_id
}

fn setup() -> (TempDir, Connection) {
    let tmp = TempDir::new().unwrap();
    let conn = db::open_database_at(&tmp.path().join("archive.db")).unwrap();
    (tmp, conn)
}

fn all(conn: &Connection) -> Vec<ArchiveObjectView> {
    search_archive_objects_filtered(conn, &SearchFilters::default()).unwrap()
}

#[test]
fn filter_by_extension_returns_only_matching_format() {
    let (_tmp, conn) = setup();
    insert_object(&conn, "doc1", Some("pdf"), "2026-01-10T10:00:00+00:00", 100);
    insert_object(&conn, "photo1", Some("jpg"), "2026-01-11T10:00:00+00:00", 200);

    let filters = SearchFilters { extension: Some("pdf".into()), ..Default::default() };
    let result = search_archive_objects_filtered(&conn, &filters).unwrap();

    assert_eq!(result.len(), 1, "мусить знайти лише один pdf");
    assert_eq!(result[0].display_name, "doc1");
}

#[test]
fn filter_by_extension_is_case_insensitive() {
    let (_tmp, conn) = setup();
    insert_object(&conn, "doc1", Some("pdf"), "2026-01-10T10:00:00+00:00", 100);

    let filters = SearchFilters { extension: Some("PDF".into()), ..Default::default() };
    let result = search_archive_objects_filtered(&conn, &filters).unwrap();

    assert_eq!(result.len(), 1, "розширення мусить ігнорувати регістр");
}

#[test]
fn filter_by_date_range_excludes_objects_outside() {
    let (_tmp, conn) = setup();
    insert_object(&conn, "january", Some("pdf"), "2026-01-15T10:00:00+00:00", 100);
    insert_object(&conn, "february", Some("pdf"), "2026-02-15T10:00:00+00:00", 100);
    insert_object(&conn, "march", Some("pdf"), "2026-03-15T10:00:00+00:00", 100);

    let filters = SearchFilters {
        date_from: Some("2026-02-01".into()),
        date_to: Some("2026-02-28".into()),
        ..Default::default()
    };
    let result = search_archive_objects_filtered(&conn, &filters).unwrap();

    assert_eq!(result.len(), 1, "лише лютий потрапляє у діапазон");
    assert_eq!(result[0].display_name, "february");
}

#[test]
fn filter_by_size_range_returns_only_middle_object() {
    let (_tmp, conn) = setup();
    insert_object(&conn, "small", Some("pdf"), "2026-01-10T10:00:00+00:00", 100);
    insert_object(&conn, "medium", Some("pdf"), "2026-01-11T10:00:00+00:00", 5000);
    insert_object(&conn, "huge", Some("pdf"), "2026-01-12T10:00:00+00:00", 1_000_000);

    let filters = SearchFilters {
        min_size: Some(1000),
        max_size: Some(100_000),
        ..Default::default()
    };
    let result = search_archive_objects_filtered(&conn, &filters).unwrap();

    assert_eq!(result.len(), 1);
    assert_eq!(result[0].display_name, "medium");
}

#[test]
fn filters_combine_with_each_other() {
    let (_tmp, conn) = setup();
    // Мусить пройти всі три фільтри одразу:
    insert_object(&conn, "report-final", Some("pdf"), "2026-02-15T10:00:00+00:00", 5000);
    // Відсіюється розміром (500 < 1000):
    insert_object(&conn, "report-draft", Some("pdf"), "2026-02-15T10:00:00+00:00", 500);
    // Відсіюється форматом (docx != pdf):
    insert_object(&conn, "letter-final", Some("docx"), "2026-02-15T10:00:00+00:00", 5000);
    // Відсіюється текстом пошуку ("photo" не містить "report"):
    insert_object(&conn, "photo-final", Some("pdf"), "2026-02-15T10:00:00+00:00", 5000);

    let filters = SearchFilters {
        query: Some("report".into()),
        extension: Some("pdf".into()),
        min_size: Some(1000),
        ..Default::default()
    };
    let result = search_archive_objects_filtered(&conn, &filters).unwrap();

    assert_eq!(result.len(), 1, "текст + формат + розмір одночасно");
    assert_eq!(result[0].display_name, "report-final");
}

#[test]
fn no_filters_returns_everything() {
    let (_tmp, conn) = setup();
    insert_object(&conn, "one", Some("pdf"), "2026-01-10T10:00:00+00:00", 100);
    insert_object(&conn, "two", Some("jpg"), "2026-02-10T10:00:00+00:00", 200);

    assert_eq!(all(&conn).len(), 2, "порожні фільтри = весь реєстр");
}
