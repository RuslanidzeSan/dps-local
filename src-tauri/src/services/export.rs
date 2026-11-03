use chrono::Utc;
use rusqlite::Connection;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

/// Одна строка вкладки «Експорти».
#[derive(Serialize)]
pub struct ExportInfoDTO {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub modified_at: String, // RFC3339
}

/// Тека зберігання експортів: ~/.dps-local/exports/
/// ВИЗНАЧЕНО В db.rs (див. 2.1а) — тут лише реекспорт для зручності.
pub use crate::db::get_exports_dir;

/// Записує файл експорту у теку. kind: "csv" | "json". Повертає шлях до файлу.
pub fn save_export_at(exports_dir: &Path, kind: &str, content: &str) -> Result<PathBuf, String> {
    let ext = match kind {
        "csv" => "csv",
        "json" => "json",
        other => return Err(format!("Невідомий тип експорту: {other}")),
    };
    fs::create_dir_all(exports_dir).map_err(|e| e.to_string())?;
    let stamp = Utc::now().format("%Y-%m-%d_%H-%M-%S");
    // Не перезаписуємо існуючі файли: якщо ім'я вже зайняте — додаємо _1, _2, ...
    let mut path = exports_dir.join(format!("export_{stamp}.{ext}"));
    let mut counter = 1;
    while path.exists() {
        path = exports_dir.join(format!("export_{stamp}_{counter}.{ext}"));
        counter += 1;
    }
    fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Список файлів експортів, новіші — першими.
pub fn list_exports_at(exports_dir: &Path) -> Result<Vec<ExportInfoDTO>, String> {
    if !exports_dir.exists() {
        return Ok(vec![]); // теки ще немає — експортів теж немає
    }
    let mut items: Vec<(std::time::SystemTime, ExportInfoDTO)> = Vec::new();
    for entry in fs::read_dir(exports_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let meta = entry.metadata().map_err(|e| e.to_string())?;
        if !meta.is_file() { continue; }
        let modified = meta.modified().map_err(|e| e.to_string())?;
        let modified_at: chrono::DateTime<Utc> = modified.into();
        items.push((
            modified,
            ExportInfoDTO {
                name: entry.file_name().to_string_lossy().to_string(),
                path: entry.path().to_string_lossy().to_string(),
                size_bytes: meta.len(),
                modified_at: modified_at.to_rfc3339(),
            },
        ));
    }
    items.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(items.into_iter().map(|(_, dto)| dto).collect())
}

/// Перейменування файлу ВНУТРІШНІХ меж теки експортів.
/// Відкидає: порожні імена, '/' та '\' (втеча з теки), існуючі назви.
pub fn rename_export_at(exports_dir: &Path, old_name: &str, new_name: &str) -> Result<(), String> {
    let clean = new_name.trim();
    if clean.is_empty() {
        return Err("Нова назва не може бути порожньою".into());
    }
    if clean.contains('/') || clean.contains('\\') || clean.contains("..") {
        return Err("Назва не може містити символи шляху".into());
    }
    let old_path = exports_dir.join(old_name);
    let new_path = exports_dir.join(clean);
    if !old_path.exists() {
        return Err("Файл не знайдено".into());
    }
    if new_path.exists() {
        return Err("Файл з такою назвою вже існує".into());
    }
    fs::rename(&old_path, &new_path).map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct ExportObject {
    pub id: String,
    pub display_name: String,
    pub original_name: Option<String>,
    pub file_extension: Option<String>,
    pub status: String,
    pub created_at: String,
    pub file_size_bytes: i64,
    pub checksum_sha256: String,
}

/// Експортує всі об'єкти у форматі CSV.
pub fn export_objects_csv(conn: &Connection) -> Result<String, String> {
    export_objects_csv_filtered(conn, None)
}

/// Експортує всі об'єкти у форматі JSON.
pub fn export_objects_json(conn: &Connection) -> Result<String, String> {
    export_objects_json_filtered(conn, None)
}

/// Експортує об'єкти у форматі CSV з фільтром за розширенням.
pub fn export_objects_csv_filtered(conn: &Connection, extension: Option<&str>) -> Result<String, String> {
    let objects = fetch_objects(conn, extension)?;
    
    let mut csv = String::from("id,display_name,original_name,file_extension,status,created_at,file_size_bytes,checksum_sha256\n");
    
    for obj in objects {
        csv.push_str(&format!(
            "{},{},{},{},{},{},{},{}\n",
            escape_csv(&obj.id),
            escape_csv(&obj.display_name),
            escape_csv(&obj.original_name.unwrap_or_default()),
            escape_csv(&obj.file_extension.unwrap_or_default()),
            escape_csv(&obj.status),
            escape_csv(&obj.created_at),
            obj.file_size_bytes,
            escape_csv(&obj.checksum_sha256)
        ));
    }
    
    Ok(csv)
}

/// Експортує об'єкти у форматі JSON з фільтром за розширенням.
pub fn export_objects_json_filtered(conn: &Connection, extension: Option<&str>) -> Result<String, String> {
    let objects = fetch_objects(conn, extension)?;
    serde_json::to_string_pretty(&objects).map_err(|e| e.to_string())
}

fn fetch_objects(conn: &Connection, extension_filter: Option<&str>) -> Result<Vec<ExportObject>, String> {
    let mut sql = String::from(
        "SELECT o.id, o.display_name, o.original_name, o.file_extension, o.status, o.created_at,
                s.file_size_bytes, COALESCE(s.checksum_sha256, '')
         FROM archive_objects o
         JOIN representations r ON r.object_id = o.id
         JOIN storage_locations s ON s.representation_id = r.id
         WHERE o.status != 'Destroyed'"
    );
    
    let mut params: Vec<String> = Vec::new();
    
    if let Some(ext) = extension_filter {
        sql.push_str(" AND o.file_extension = ?");
        params.push(ext.to_string());
    }
    
    sql.push_str(" ORDER BY o.created_at DESC");
    
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    
    let objects = stmt
        .query_map(rusqlite::params_from_iter(params), |row| {
            Ok(ExportObject {
                id: row.get(0)?,
                display_name: row.get(1)?,
                original_name: row.get(2)?,
                file_extension: row.get(3)?,
                status: row.get(4)?,
                created_at: row.get(5)?,
                file_size_bytes: row.get(6)?,
                checksum_sha256: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    
    Ok(objects)
}

fn escape_csv(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}
