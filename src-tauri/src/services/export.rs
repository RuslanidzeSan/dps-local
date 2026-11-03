//! Сервіс експорту метаданих архіву у CSV та JSON.

use rusqlite::Connection;
use serde::Serialize;

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
