use dps_local_lib::db;
use dps_local_lib::services::thumbnails::{
    generate_thumbnail_at, generate_missing_thumbnails_at, get_thumbnail_base64_at,
    is_image_extension,
};
use rusqlite::{params, Connection};
use std::path::Path;
use tempfile::TempDir;

fn setup() -> (TempDir, Connection) {
    let tmp = TempDir::new().unwrap();
    let conn = db::open_database_at(&tmp.path().join("archive.db")).unwrap();
    (tmp, conn)
}

/// Малює справжній тестовий PNG 40×30 пікселів.
fn write_test_png(path: &Path) {
    let img = image::RgbImage::from_pixel(40, 30, image::Rgb([200, 30, 30]));
    img.save_with_format(path, image::ImageFormat::Png).unwrap();
}

/// Реєструє managed-об'єкт, чиїм "файлом сховища" є реальний файл у tmp.
fn insert_object(conn: &Connection, tmp: &Path, name: &str, ext: &str, make_file: bool) -> String {
    let object_id = format!("obj-{name}");
    let created_at = "2026-01-10T10:00:00+00:00";
    let file_path = tmp.join(format!("{name}.bin"));
    if make_file { std::fs::write(&file_path, b"fake").unwrap(); }
    conn.execute(
        "INSERT INTO archive_objects(id,display_name,original_name,file_extension,status,document_family_id,created_at,updated_at) VALUES(?1,?2,?2,?3,'Active',?1,?4,?4)",
        params![object_id, name, ext, created_at],
    ).unwrap();
    conn.execute(
        "INSERT INTO representations(id,object_id,representation_type,created_at) VALUES(?1,?2,'original',?3)",
        params![format!("rep-{name}"), object_id, created_at],
    ).unwrap();
    conn.execute(
        "INSERT INTO storage_locations(id,representation_id,storage_type,file_path,file_size_bytes,created_at) VALUES(?1,?2,'managed',?3,4,?4)",
        params![format!("loc-{name}"), format!("rep-{name}"), file_path.to_string_lossy(), created_at],
    ).unwrap();
    object_id
}

#[test]
fn is_image_extension_detects_formats() {
    assert!(is_image_extension(Some("jpg")));
    assert!(is_image_extension(Some("JPG")));
    assert!(is_image_extension(Some(".png")));
    assert!(!is_image_extension(Some("txt")));
    assert!(!is_image_extension(None));
}

#[test]
fn thumbnail_created_for_real_image() {
    let (tmp, _conn) = setup();
    let src = tmp.path().join("photo.png");
    write_test_png(&src);
    let dest = tmp.path().join("thumbs").join("photo.jpg");

    let created = generate_thumbnail_at(&src, &dest).unwrap();

    assert!(created, "для PNG мініатюра мусить створитися");
    assert!(dest.is_file(), "файл мініатюри існує");
    let bytes = std::fs::read(&dest).unwrap();
    assert_eq!(&bytes[0..2], &[0xFF, 0xD8], "файл мусить бути справжнім JPEG");
}

#[test]
fn thumbnail_skips_non_image_files() {
    let (tmp, _conn) = setup();
    let src = tmp.path().join("doc.txt");
    std::fs::write(&src, b"just text").unwrap();
    let dest = tmp.path().join("thumbs").join("doc.jpg");

    let created = generate_thumbnail_at(&src, &dest).unwrap();

    assert!(!created, "текстовий файл не мусить давати мініатюру");
    assert!(!dest.exists());
}

#[test]
fn backfill_generates_only_for_real_images() {
    let (tmp, conn) = setup();
    // Реальний PNG + два "не-фото" (txt з файлом, png без файла на диску)
    let png_path = tmp.path().join("photo.png");
    write_test_png(&png_path);
    let obj_photo = insert_object(&conn, tmp.path(), "photo", "png", false);
    // перезапишемо шлях на справжній PNG
    conn.execute("UPDATE storage_locations SET file_path=?1 WHERE id=?2",
        params![png_path.to_string_lossy(), "loc-photo"]).unwrap();
    insert_object(&conn, tmp.path(), "notes", "txt", true);
    insert_object(&conn, tmp.path(), "ghost", "png", true);

    let thumbs_dir = tmp.path().join("thumbs");
    let generated = generate_missing_thumbnails_at(&conn, &thumbs_dir).unwrap();

    assert_eq!(generated, 1, "лише photo.png має мініатюру");
    let data = get_thumbnail_base64_at(&thumbs_dir, &obj_photo).unwrap();
    assert!(data.is_some(), "мініатюра photo читається");
    assert!(data.unwrap().starts_with("data:image/jpeg;base64,"));
    assert_eq!(get_thumbnail_base64_at(&thumbs_dir, "obj-notes").unwrap(), None);
}

#[test]
fn backfill_skips_existing_and_is_idempotent() {
    let (tmp, conn) = setup();
    let png_path = tmp.path().join("photo.png");
    write_test_png(&png_path);
    insert_object(&conn, tmp.path(), "photo", "png", false);
    conn.execute("UPDATE storage_locations SET file_path=?1 WHERE id=?2",
        params![png_path.to_string_lossy(), "loc-photo"]).unwrap();

    let thumbs_dir = tmp.path().join("thumbs");
    let first = generate_missing_thumbnails_at(&conn, &thumbs_dir).unwrap();
    let second = generate_missing_thumbnails_at(&conn, &thumbs_dir).unwrap();

    assert_eq!(first, 1);
    assert_eq!(second, 0, "повторний запуск не мусить дублювати роботу");
}

#[test]
fn get_thumbnail_returns_none_for_unknown_object() {
    let (tmp, _conn) = setup();
    assert_eq!(get_thumbnail_base64_at(&tmp.path().join("thumbs"), "obj-nope").unwrap(), None);
}
