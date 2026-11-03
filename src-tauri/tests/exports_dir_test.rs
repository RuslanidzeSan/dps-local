use dps_local_lib::services::export::{list_exports_at, rename_export_at, save_export_at};
use std::fs;
use std::path::PathBuf;

fn temp_exports_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dps_exports_test_{}_{}", name, std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn save_export_writes_file_and_returns_path() {
    let dir = temp_exports_dir("save");
    let path = save_export_at(&dir, "csv", "id,name\n1,тест").expect("save failed");

    assert!(path.exists(), "файл експорту має існувати");
    assert!(path.to_string_lossy().ends_with(".csv"), "розширення має відповідати kind");
    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, "id,name\n1,тест");
    // Шлях — саме всередині заданої теки
    assert!(path.starts_with(&dir), "файл має лежати в теці експортів");
}

#[test]
fn list_exports_returns_metadata_newest_first() {
    let dir = temp_exports_dir("list");
    save_export_at(&dir, "csv", "a").unwrap();
    // невелика пауза, щоб мітки часу відрізнялися
    std::thread::sleep(std::time::Duration::from_millis(1100));
    save_export_at(&dir, "json", "[]").unwrap();

    let items = list_exports_at(&dir).expect("list failed");
    assert_eq!(items.len(), 2, "мало бути два файли");
    // Новіший — перший
    assert!(items[0].name.ends_with(".json"), "першим має бути новіший файл");
    assert!(items[0].name.contains("export"), "ім'я генерується програмою");
    assert!(items[0].size_bytes > 0);
    assert!(!items[0].modified_at.is_empty());
    assert_eq!(items[1].name.ends_with(".csv"), true);
}

#[test]
fn rename_export_rejects_dangerous_and_duplicate_names() {
    let dir = temp_exports_dir("rename");
    let p1 = save_export_at(&dir, "csv", "a").unwrap();
    let old_name = p1.file_name().unwrap().to_string_lossy().to_string();

    // 1) Ім'я з роздільником шляхів — спроба втечі з теки: має бути відхилено
    let evil = rename_export_at(&dir, &old_name, "../../evil.csv");
    assert!(evil.is_err(), "ім'я з '/' або '..' має бути відхилено");

    // 2) Перейменування на вже існуючу назву — відхилено
    save_export_at(&dir, "csv", "b").unwrap();
    let items = list_exports_at(&dir).unwrap();
    let other = items.iter().find(|i| i.name != old_name).unwrap().name.clone();
    let dup = rename_export_at(&dir, &old_name, &other);
    assert!(dup.is_err(), "перейменування на існуюче ім'я має бути відхилено");

    // 3) Валідне перейменування — працює
    rename_export_at(&dir, &old_name, "my_report.csv").expect("valid rename failed");
    let items = list_exports_at(&dir).unwrap();
    assert!(items.iter().any(|i| i.name == "my_report.csv"), "нове ім'я у списку");
    assert!(!items.iter().any(|i| i.name == old_name), "старе ім'я зникло");
}
