use crate::db;
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};

/// Створює повну резервну копію архіву у backup_dir.
/// Повертає шлях до створеної папки копії (dps-backup-<дата-час>).
pub fn create_backup_at(db_path: &Path, storage_dir: &Path, backup_dir: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(backup_dir).map_err(|e| e.to_string())?;

    // 1) Ім'я папки з тимчасовою міткою UTC
    let timestamp = chrono::Utc::now().format("%Y-%m-%dT%H-%M-%SZ").to_string();
    let backup_path = backup_dir.join(format!("dps-backup-{timestamp}"));
    fs::create_dir_all(&backup_path).map_err(|e| e.to_string())?;

    // 2) Цілісний снапшот бази через VACUUM INTO (не просте копіювання!)
    let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    let snapshot = backup_path.join("archive.db");
    let snapshot_str = snapshot.to_string_lossy().to_string();
    conn.execute("VACUUM INTO ?1", [snapshot_str.as_str()])
        .map_err(|e| format!("VACUUM INTO failed: {e}"))?;
    drop(conn); // закриваємо до копіювання storage

    // 3) Копіюємо файли сховища
    let backup_storage = backup_path.join("storage");
    copy_dir_recursive(storage_dir, &backup_storage)?;

    // Експорти і мініатюри лежать поруч з базою — копіюємо їх також
    // (якщо тек немає, copy_dir_recursive нічого не робить)
    if let Some(data_dir) = db_path.parent() {
        let exports_dir = data_dir.join("exports");
        copy_dir_recursive(&exports_dir, &backup_path.join("exports"))?;
        let thumbs_dir = data_dir.join("thumbnails");
        copy_dir_recursive(&thumbs_dir, &backup_path.join("thumbnails"))?;
    }

    // 4) manifest.json — «паспорт» копії
    let files_count = count_files(&backup_storage);
    let manifest = serde_json::json!({
        "app": "dps-local",
        "created_at": chrono::Utc::now().to_rfc3339(),
        "database": "archive.db",
        "storage_dir": "storage",
        "storage_files": files_count,
    });
    fs::write(
        backup_path.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    Ok(backup_path)
}

/// Зручна обгортка для Tauri-команди: працює зі шляхами за замовчуванням.
pub fn create_backup_default(backup_dir: &Path) -> Result<PathBuf, String> {
    create_backup_at(&db::get_database_path(), &db::get_storage_dir(), backup_dir)
}

/// Відновлює архів із папки копії. Атомарна заміна бази через .tmp + rename.
pub fn restore_backup_at(backup_path: &Path, db_path: &Path, storage_dir: &Path) -> Result<(), String> {
    // 1) Валідація: без manifest.json відновлюватись відмовляємось
    let manifest_path = backup_path.join("manifest.json");
    if !manifest_path.exists() {
        return Err("Це не схоже на резервну копію dps-local: відсутній manifest.json".into());
    }
    let snapshot_db = backup_path.join("archive.db");
    if !snapshot_db.exists() {
        return Err("У резервній копії немає файлу бази archive.db".into());
    }

    // 2) Атомарна заміна бази: спочатку копія поруч як .tmp, потім rename
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = db_path.with_extension("db.restore-tmp");
    fs::copy(&snapshot_db, &tmp).map_err(|e| format!("не вдалося скопіювати базу: {e}"))?;
    fs::rename(&tmp, db_path).map_err(|e| format!("не вдалося замінити базу: {e}"))?;

    // 3) Файли сховища: додаємо/перезаписуємо, зайвих НЕ видаляємо (рішення MVP)
    copy_dir_recursive(&backup_path.join("storage"), storage_dir)?;

// Мініатюри: повертаємо з копії поруч з базою
    if let Some(data_dir) = db_path.parent() {
        let thumbs = data_dir.join("thumbnails");
        copy_dir_recursive(&backup_path.join("thumbnails"), &thumbs)?;
    }

    Ok(())
}

/// Рекурсивне копіювання каталогу (якщо джерела немає — окремо нічого не робимо).
fn copy_dir_recursive(from: &Path, to: &Path) -> Result<(), String> {
    if !from.exists() {
        return Ok(()); // порожнє сховище — це теж валідний випадок
    }
    fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(from).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let target = to.join(entry.file_name());
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), &target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn count_files(dir: &Path) -> usize {
    fs::read_dir(dir)
        .map(|entries| entries.filter_map(|e| e.ok()).filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false)).count())
        .unwrap_or(0)
}
