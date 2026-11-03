//! Інтеграційні тести для імпорту каталогів (batch ingest).
//! Запуск: npm run test:rust

use dps_local_lib::db::open_database_at;
use dps_local_lib::services::ingest::ingest_directory;
use std::fs;

fn setup() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf, rusqlite::Connection) {
    let temp = tempfile::tempdir().unwrap();
    let source_dir = temp.path().join("documents");
    let storage = temp.path().join("storage");
    let db_path = temp.path().join("archive.db");
    fs::create_dir(&source_dir).unwrap();
    let conn = open_database_at(&db_path).unwrap();
    (temp, source_dir, storage, conn)
}

#[test]
fn ingest_directory_imports_all_files() {
    let (_temp, source_dir, storage, mut conn) = setup();

    // Створюємо тестові файли
    fs::write(source_dir.join("doc1.txt"), b"Document 1").unwrap();
    fs::write(source_dir.join("doc2.txt"), b"Document 2").unwrap();
    fs::write(source_dir.join("doc3.pdf"), b"PDF content").unwrap();

    let count = ingest_directory(
        &mut conn,
        &source_dir,
        &storage,
        "00000000-0000-4000-8000-000000000001", false
    )
    .unwrap();

    assert_eq!(count, 3, "має бути імпортовано 3 файли");

    // Перевіряємо, що всі файли в базі
    let objects: i64 = conn
        .query_row("SELECT COUNT(*) FROM archive_objects", [], |r| r.get(0))
        .unwrap();
    assert_eq!(objects, 3);

    // Перевіряємо, що файли скопіювалися в сховище
    let copies: i64 = conn
        .query_row("SELECT COUNT(*) FROM storage_locations", [], |r| r.get(0))
        .unwrap();
    assert_eq!(copies, 3);

    // Перевіряємо, що оригінали не змінилися
    assert_eq!(fs::read(source_dir.join("doc1.txt")).unwrap(), b"Document 1");
}

#[test]
fn ingest_directory_skips_subdirectories_and_hidden_files() {
    let (_temp, source_dir, storage, mut conn) = setup();

    // Створюємо файли
    fs::write(source_dir.join("visible.txt"), b"Visible").unwrap();
    fs::write(source_dir.join(".hidden"), b"Hidden").unwrap();

    // Створюємо підкаталог з файлом
    let subdir = source_dir.join("subdir");
    fs::create_dir(&subdir).unwrap();
    fs::write(subdir.join("nested.txt"), b"Nested").unwrap();

    let count = ingest_directory(
        &mut conn,
        &source_dir,
        &storage,
        "00000000-0000-4000-8000-000000000001", false
    )
    .unwrap();

    assert_eq!(count, 1, "має бути імпортовано лише 1 файл (visible.txt)");
}

#[test]
fn ingest_directory_handles_empty_directory() {
    let (_temp, source_dir, storage, mut conn) = setup();

    let count = ingest_directory(
        &mut conn,
        &source_dir,
        &storage,
        "00000000-0000-4000-8000-000000000001", false
    )
    .unwrap();

    assert_eq!(count, 0, "порожній каталог має повертати 0");
}

#[test]
fn ingest_directory_continues_on_individual_file_errors() {
    let (_temp, source_dir, storage, mut conn) = setup();

    // Створюємо файл, який зникне під час імпорту (симуляція помилки)
    fs::write(source_dir.join("good.txt"), b"Good file").unwrap();
    fs::write(source_dir.join("bad.txt"), b"Bad file").unwrap();

    // Видаляємо файл після створення, але перед імпортом
    fs::remove_file(source_dir.join("bad.txt")).unwrap();

    let count = ingest_directory(
        &mut conn,
        &source_dir,
        &storage,
        "00000000-0000-4000-8000-000000000001", false
    )
    .unwrap();

    assert_eq!(count, 1, "лише good.txt має бути імпортовано");
}

#[test]
fn ingest_directory_recursive_imports_subdirectories() {
    let (_temp, source_dir, storage, mut conn) = setup();

    // Створюємо файли в корені
    fs::write(source_dir.join("root.txt"), b"Root file").unwrap();

    // Створюємо підпапку з файлами
    let subdir = source_dir.join("subdir");
    fs::create_dir(&subdir).unwrap();
    fs::write(subdir.join("nested1.txt"), b"Nested 1").unwrap();
    fs::write(subdir.join("nested2.txt"), b"Nested 2").unwrap();

    // Створюємо ще одну рівень вкладеності
    let deep_dir = subdir.join("deep");
    fs::create_dir(&deep_dir).unwrap();
    fs::write(deep_dir.join("deep.txt"), b"Deep file").unwrap();

    // Імпортуємо рекурсивно
    let count = ingest_directory(
        &mut conn,
        &source_dir,
        &storage,
        "00000000-0000-4000-8000-000000000001",
        true, // recursive = true
    )
    .unwrap();

    assert_eq!(count, 4, "має бути імпортовано 4 файли (1 корінь + 2 підпапка + 1 глибоко)");
}
