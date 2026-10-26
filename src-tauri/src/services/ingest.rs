use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::Path;
use uuid::Uuid;
use chrono::Utc;

pub fn process_ingest(
    conn: &mut Connection,
    source_path: &Path,
    storage_dir: &Path,
    user_id: &str,
) -> Result<String, String> {
    if !source_path.exists() {
        return Err("Source file does not exist".to_string());
    }

    if !storage_dir.exists() {
        fs::create_dir_all(storage_dir).map_err(|e| e.to_string())?;
    }

    let file_name = source_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    
    let object_id = Uuid::new_v4().to_string();

    // 1. Копіюємо як *.tmp (згідно з ТЗ)
    let temp_dest_path = storage_dir.join(format!("{}.tmp", object_id));
    let file_size = fs::copy(source_path, &temp_dest_path).map_err(|e| e.to_string())?;

    // 2. Обчислення SHA-256
    let mut file = fs::File::open(&temp_dest_path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    let hash = hex::encode(hasher.finalize());

    // 3. Атомарно перейменовуємо файл на фінальну назву
    let final_dest_filename = format!("{}_{}", object_id, file_name);
    let final_dest_path = storage_dir.join(&final_dest_filename);
    fs::rename(&temp_dest_path, &final_dest_path).map_err(|e| e.to_string())?;

    // 4. Транзакція БД (все або нічого)
    let representation_id = Uuid::new_v4().to_string();
    let location_id = Uuid::new_v4().to_string();
    let job_id = Uuid::new_v4().to_string();
    let audit_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

    tx.execute(
        "INSERT INTO ingest_jobs (id, status, source_path, created_at, completed_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        (&job_id, "completed", source_path.to_string_lossy(), &now, &now),
    ).map_err(|e| e.to_string())?;

    tx.execute(
        "INSERT INTO archive_objects (id, display_name, original_name, status, document_family_id, created_at, updated_at) 
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        (&object_id, &file_name, &file_name, "Active", &object_id, &now, &now),
    ).map_err(|e| e.to_string())?;

    tx.execute(
        "INSERT INTO representations (id, object_id, representation_type, created_at) VALUES (?1, ?2, ?3, ?4)",
        (&representation_id, &object_id, "original", &now),
    ).map_err(|e| e.to_string())?;

    tx.execute(
        "INSERT INTO storage_locations (id, representation_id, storage_type, file_path, file_size_bytes, checksum_sha256, is_accessible, created_at) 
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        (&location_id, &representation_id, "managed", final_dest_path.to_string_lossy(), file_size as i64, &hash, true, &now),
    ).map_err(|e| e.to_string())?;

    tx.execute(
        "INSERT INTO audit_events (id, user_id, action, target_table, target_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        (&audit_id, user_id, "INGEST_COMPLETED", "archive_objects", &object_id, &now),
    ).map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;

    Ok(object_id)
}
