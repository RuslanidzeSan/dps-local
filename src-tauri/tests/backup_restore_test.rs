use dps_local_lib::db::open_database_at;
use dps_local_lib::services::backup::{create_backup_at, restore_backup_at};
use rusqlite::Connection;
use std::fs;
use std::path::PathBuf;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dps_backup_test_{}_{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Створює «живий архів»: база з одним об'єктом + один файл у storage.
fn make_live_archive(root: &PathBuf) -> (PathBuf, PathBuf) {
    let db_path = root.join("archive.db");
    let storage_dir = root.join("storage");
    fs::create_dir_all(&storage_dir).unwrap();

    let conn = open_database_at(&db_path).unwrap();
    // Один об'єкт напряму в SQL — щоб тест не залежав від процесу імпорту
    conn.execute(
        "INSERT INTO archive_objects (id, display_name, original_name, file_extension, status, document_family_id, created_at, updated_at)
         VALUES ('33333333-3333-3333-3333-333333333333', 'Тестова картка', 'test.pdf', 'pdf', 'Active',
                 '44444444-4444-4444-4444-444444444444',
                 '2026-11-03T10:00:00Z', '2026-11-03T10:00:00Z')",
        [],
    )
    .unwrap();
    fs::write(storage_dir.join("test.pdf"), b"pretend this is a pdf").unwrap();
    (db_path, storage_dir)
}

#[test]
fn backup_creates_complete_snapshot() {
    let root = temp_dir("backup");
    let (db_path, storage_dir) = make_live_archive(&root);
    let backup_root = root.join("backups");

    let backup_path = create_backup_at(&db_path, &storage_dir, &backup_root).expect("backup failed");

    // 1) Структура копії
    assert!(backup_path.join("archive.db").exists(), "у копії має бути база");
    assert!(backup_path.join("storage/test.pdf").exists(), "у копії має бути файл сховища");
    assert!(backup_path.join("manifest.json").exists(), "у копії має бути manifest.json");

    // 2) База в копії — цілісна і з нашою карткою (перевіряємо саме КОПІЮ, не оригінал)
    let backup_conn = Connection::open(backup_path.join("archive.db")).unwrap();
    let count: i64 = backup_conn
        .query_row("SELECT COUNT(*) FROM archive_objects WHERE id = '33333333-3333-3333-3333-333333333333'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1, "копія бази має містити картку");

    // 3) Оригінал не постраждав
    let orig = Connection::open(&db_path).unwrap();
    let orig_count: i64 = orig.query_row("SELECT COUNT(*) FROM archive_objects", [], |r| r.get(0)).unwrap();
    assert_eq!(orig_count, 1);
}

#[test]
fn restore_recovers_after_data_loss() {
    let root = temp_dir("restore");
    let (db_path, storage_dir) = make_live_archive(&root);
    let backup_root = root.join("backups");
    let backup_path = create_backup_at(&db_path, &storage_dir, &backup_root).unwrap();

    // СИМУЛЯЦІЯ КРИЗИ: база і файли знищені
    fs::remove_file(&db_path).unwrap();
    fs::remove_file(storage_dir.join("test.pdf")).unwrap();

    restore_backup_at(&backup_path, &db_path, &storage_dir).expect("restore failed");

    // База відновлена і читається
    let conn = open_database_at(&db_path).unwrap();
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM archive_objects WHERE id = '33333333-3333-3333-3333-333333333333'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1, "картка має повернутися після restore");

    // Файл сховища відновлений, байт-у-байт
    let bytes = fs::read(storage_dir.join("test.pdf")).unwrap();
    assert_eq!(bytes, b"pretend this is a pdf");

    // Копія не знищилась під час restore
    assert!(backup_path.join("archive.db").exists());
}

#[test]
fn restore_rejects_folder_without_manifest() {
    let root = temp_dir("bad_restore");
    let (db_path, storage_dir) = make_live_archive(&root);

    // «Фальшива» папка копії: база є, manifest.json немає
    let fake = root.join("fake_backup");
    fs::create_dir_all(&fake).unwrap();
    fs::write(fake.join("archive.db"), b"junk").unwrap();

    let result = restore_backup_at(&fake, &db_path, &storage_dir);
    assert!(result.is_err(), "restore без manifest.json має бути відхилено");
}

#[test]
fn backup_and_restore_include_thumbnails() {
    let root = temp_dir("thumbs");
    let (db_path, storage_dir) = make_live_archive(&root);

    // У «живому архіві» є тека мініатюр з одним файлом
    let thumbs_dir = root.join("thumbnails");
    fs::create_dir_all(&thumbs_dir).unwrap();
    fs::write(thumbs_dir.join("33333333-3333-3333-3333-333333333333.jpg"), b"fake thumbnail").unwrap();

    let backup_path = create_backup_at(&db_path, &storage_dir, &root.join("backups")).unwrap();
    assert!(backup_path.join("thumbnails/33333333-3333-3333-3333-333333333333.jpg").exists(),
        "у копії має бути мініатюра");

    // СИМУЛЯЦІЯ КРИЗИ: мініатюри знищено
    fs::remove_dir_all(&thumbs_dir).unwrap();

    restore_backup_at(&backup_path, &db_path, &storage_dir).unwrap();
    assert!(thumbs_dir.join("33333333-3333-3333-3333-333333333333.jpg").exists(),
        "мініатюра має повернутися після restore");
}
