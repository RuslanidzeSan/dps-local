use crate::db::LOCAL_USER_ID;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::Path;
use uuid::Uuid;

#[derive(serde::Serialize)]
pub struct IngestResult { pub object_id: String, pub duplicate_detected: bool }

pub fn process_ingest(conn: &mut Connection, source_path: &Path, storage_dir: &Path, user_id: &str) -> Result<IngestResult, String> {
    if !source_path.is_file() { return Err("Вибраний шлях не є доступним файлом".into()); }
    fs::create_dir_all(storage_dir).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(storage_dir).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_some_and(|x| x == "tmp") { let _ = fs::remove_file(path); }
    }
    let object_id = Uuid::new_v4().to_string();
    let job_id = Uuid::new_v4().to_string(); let now = Utc::now().to_rfc3339();
    conn.execute("INSERT INTO ingest_jobs(id,status,source_path,created_at) VALUES(?1,'processing',?2,?3)", params![job_id, source_path.to_string_lossy(), now]).map_err(|e| e.to_string())?;
    let original_name = source_path.file_name().ok_or("Файл не має назви")?.to_string_lossy().into_owned();
    let extension = source_path.extension().map(|x| x.to_string_lossy().to_ascii_lowercase());
    let temp_path = storage_dir.join(format!("{object_id}.tmp"));
    let final_path = storage_dir.join(format!("{object_id}_{original_name}"));
    let result = (|| -> Result<IngestResult, String> {
        let file_size = fs::copy(source_path, &temp_path).map_err(|e| e.to_string())?;
        let checksum = sha256_file(&temp_path)?;
        let duplicate_detected: bool = conn.query_row("SELECT EXISTS(SELECT 1 FROM storage_locations WHERE checksum_sha256=?1)", [&checksum], |r| r.get(0)).map_err(|e| e.to_string())?;
        fs::rename(&temp_path, &final_path).map_err(|e| e.to_string())?;
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let representation_id = Uuid::new_v4().to_string(); let location_id = Uuid::new_v4().to_string();
        tx.execute("INSERT INTO archive_objects(id,display_name,original_name,file_extension,status,document_family_id,created_at,updated_at) VALUES(?1,?2,?3,?4,'Active',?1,?5,?5)", params![object_id, original_name, original_name, extension, now]).map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO representations(id,object_id,representation_type,created_at) VALUES(?1,?2,'original',?3)", params![representation_id, object_id, now]).map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO storage_locations(id,representation_id,storage_type,file_path,file_size_bytes,checksum_sha256,created_at) VALUES(?1,?2,'managed',?3,?4,?5,?6)", params![location_id, representation_id, final_path.to_string_lossy(), file_size as i64, checksum, now]).map_err(|e| e.to_string())?;
        tx.execute("UPDATE ingest_jobs SET status='completed',source_sha256=?1,object_id=?2,completed_at=?3 WHERE id=?4", params![checksum, object_id, now, job_id]).map_err(|e| e.to_string())?;
        let payload = serde_json::json!({"source": source_path, "duplicate_detected": duplicate_detected}).to_string();
        tx.execute("INSERT INTO audit_events(id,user_id,action,target_table,target_id,payload,created_at) VALUES(?1,?2,'INGEST_COMPLETED','archive_objects',?3,?4,?5)", params![Uuid::new_v4().to_string(), user_id, object_id, payload, now]).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
        Ok(IngestResult { object_id, duplicate_detected })
    })();
    if let Err(error) = &result {
        let _ = fs::remove_file(&temp_path); let _ = fs::remove_file(&final_path);
        let _ = conn.execute("UPDATE ingest_jobs SET status='failed',completed_at=?1,error_message=?2 WHERE id=?3", params![Utc::now().to_rfc3339(), error, job_id]);
    }
    result
}

/// Registers an external file without copying or modifying it. Unlike managed
/// ingest, its availability is not guaranteed and is exposed to the UI.
pub fn index_reference(conn: &mut Connection, source_path: &Path, user_id: &str) -> Result<IngestResult, String> {
    if !source_path.is_file() { return Err("Вибраний зовнішній файл недоступний".into()); }
    let object_id = Uuid::new_v4().to_string();
    let representation_id = Uuid::new_v4().to_string();
    let location_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let original_name = source_path.file_name().ok_or("Файл не має назви")?.to_string_lossy().into_owned();
    let extension = source_path.extension().map(|x| x.to_string_lossy().to_ascii_lowercase());
    let size = fs::metadata(source_path).map_err(|e| e.to_string())?.len() as i64;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute("INSERT INTO archive_objects(id,display_name,original_name,file_extension,status,document_family_id,created_at,updated_at) VALUES(?1,?2,?3,?4,'Active',?1,?5,?5)", params![object_id, original_name, original_name, extension, now]).map_err(|e| e.to_string())?;
    tx.execute("INSERT INTO representations(id,object_id,representation_type,created_at) VALUES(?1,?2,'original',?3)", params![representation_id, object_id, now]).map_err(|e| e.to_string())?;
    tx.execute("INSERT INTO storage_locations(id,representation_id,storage_type,file_path,file_size_bytes,checksum_sha256,is_accessible,created_at) VALUES(?1,?2,'reference',?3,?4,NULL,1,?5)", params![location_id, representation_id, source_path.to_string_lossy(), size, now]).map_err(|e| e.to_string())?;
    tx.execute("INSERT INTO audit_events(id,user_id,action,target_table,target_id,payload,created_at) VALUES(?1,?2,'REFERENCE_INDEXED','archive_objects',?3,?4,?5)", params![Uuid::new_v4().to_string(), user_id, object_id, source_path.to_string_lossy(), now]).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(IngestResult { object_id, duplicate_detected: false })
}

pub fn reconcile_reference_locations(conn: &mut Connection) -> Result<usize, String> {
    // Виносимо вибірку у внутрішній блок, щоб stmt гарантовано знищився (dropped) ДО транзакції
    let locations = {
        let mut stmt = conn.prepare("SELECT id, file_path, is_accessible FROM storage_locations WHERE storage_type='reference'")
            .map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?))
        }).map_err(|e| e.to_string())?;

        let mut vec = Vec::new();
        for row in rows {
            vec.push(row.map_err(|e| e.to_string())?);
        }
        vec
    }; // <-- Тут stmt закривається, з'єднання `conn` стає вільним для мутабельного запозичення

    let tx = conn.transaction().map_err(|e| e.to_string())?; 
    let mut changed = 0;
    
    for (id, path, previous) in locations {
        let accessible = i64::from(Path::new(&path).is_file());
        if accessible != previous {
            tx.execute("UPDATE storage_locations SET is_accessible=?1 WHERE id=?2", params![accessible, id])
                .map_err(|e| e.to_string())?;
            tx.execute(
                "INSERT INTO audit_events(id, user_id, action, target_table, target_id, payload, created_at) VALUES(?1, ?2, 'REFERENCE_AVAILABILITY_CHANGED', 'storage_locations', ?3, ?4, ?5)",
                params![Uuid::new_v4().to_string(), LOCAL_USER_ID, id, accessible.to_string(), Utc::now().to_rfc3339()]
            ).map_err(|e| e.to_string())?;
            changed += 1;
        }
    }
    
    tx.commit().map_err(|e| e.to_string())?; 
    Ok(changed)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?; let mut hasher = Sha256::new(); let mut buffer=[0;8192];
    loop { let n=file.read(&mut buffer).map_err(|e|e.to_string())?; if n==0 {break;} hasher.update(&buffer[..n]); }
    Ok(hex::encode(hasher.finalize()))
}

pub fn verify_object_integrity(conn: &Connection, object_id: &str) -> Result<bool, String> {
    let row: Option<(String,String)> = conn.query_row("SELECT s.file_path,s.checksum_sha256 FROM storage_locations s JOIN representations r ON r.id=s.representation_id WHERE r.object_id=?1 AND s.storage_type='managed'", [object_id], |r| Ok((r.get(0)?,r.get(1)?))).optional().map_err(|e|e.to_string())?;
    let (path, expected)=row.ok_or("Не знайдено керованої копії")?; let actual=sha256_file(Path::new(&path)); let ok=actual.as_deref()==Ok(expected.as_str());
    conn.execute("INSERT INTO audit_events(id,user_id,action,target_table,target_id,payload,created_at) VALUES(?1,?2,?3,'archive_objects',?4,?5,?6)", params![Uuid::new_v4().to_string(),LOCAL_USER_ID,if ok{"FIXITY_OK"}else{"FIXITY_FAILED"},object_id,actual.unwrap_or_else(|e|e),Utc::now().to_rfc3339()]).map_err(|e|e.to_string())?;
    Ok(ok)
}

pub fn update_object_description(conn: &mut Connection, object_id: &str, description: &str) -> Result<(), String> {
    let tx=conn.transaction().map_err(|e|e.to_string())?;
    if tx.execute("UPDATE archive_objects SET description=?1,updated_at=?2 WHERE id=?3",params![description,Utc::now().to_rfc3339(),object_id]).map_err(|e|e.to_string())? != 1 { return Err("Об'єкт не знайдено".into()); }
    tx.execute("INSERT INTO audit_events(id,user_id,action,target_table,target_id,payload,created_at) VALUES(?1,?2,'METADATA_UPDATED','archive_objects',?3,?4,?5)",params![Uuid::new_v4().to_string(),LOCAL_USER_ID,object_id,description,Utc::now().to_rfc3339()]).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())
}

pub fn search_archive_objects(conn:&Connection,query:Option<&str>,tag_id:Option<&str>,person_id:Option<&str>,location_id:Option<&str>)->Result<Vec<ArchiveObjectView>,String>{
    let mut sql=String::from("SELECT DISTINCT o.id,o.display_name,o.status,o.description,s.file_path,s.file_size_bytes,COALESCE(s.checksum_sha256,''),s.storage_type,s.is_accessible,o.created_at FROM archive_objects o JOIN representations r ON r.object_id=o.id JOIN storage_locations s ON s.representation_id=r.id LEFT JOIN map_file_tags mt ON mt.file_id=o.id LEFT JOIN map_file_persons mp ON mp.file_id=o.id LEFT JOIN map_file_locations ml ON ml.file_id=o.id WHERE o.status!='Destroyed'"); let mut args:Vec<String>=vec![];
    if let Some(q)=query.filter(|x|!x.trim().is_empty()){sql.push_str(" AND (o.display_name LIKE ? OR o.original_name LIKE ? OR o.description LIKE ? OR o.file_extension LIKE ?)");let p=format!("%{}%",q.trim());args.extend([p.clone(),p.clone(),p.clone(),p]);}
    for (value,column) in [(tag_id,"mt.tag_id"),(person_id,"mp.person_id"),(location_id,"ml.location_id")] {if let Some(v)=value.filter(|x|!x.is_empty()){sql.push_str(&format!(" AND {column}=?"));args.push(v.into());}}
    sql.push_str(" ORDER BY o.created_at DESC LIMIT 100");let mut stmt=conn.prepare(&sql).map_err(|e|e.to_string())?;
    stmt.query_map(rusqlite::params_from_iter(args),|r|Ok(ArchiveObjectView{id:r.get(0)?,display_name:r.get(1)?,status:r.get(2)?,description:r.get(3)?,file_path:r.get(4)?,file_size_bytes:r.get(5)?,checksum_sha256:r.get(6)?,storage_type:r.get(7)?,is_accessible:r.get::<_,i64>(8)? != 0,created_at:r.get(9)?})).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())
}

#[derive(serde::Serialize)] pub struct ArchiveObjectView {pub id:String,pub display_name:String,pub status:String,pub description:Option<String>,pub file_path:String,pub file_size_bytes:i64,pub checksum_sha256:String,pub storage_type:String,pub is_accessible:bool,pub created_at:String}

/// Імпортує файли з каталогу.
/// Якщо `recursive` = true — також обробляє всі підкаталоги.
/// Повертає кількість успішно імпортованих файлів.
pub fn ingest_directory(
    conn: &mut Connection,
    dir_path: &Path,
    storage_dir: &Path,
    user_id: &str,
    recursive: bool,
) -> Result<usize, String> {
    if !dir_path.is_dir() {
        return Err("Вибраний шлях не є каталогом".into());
    }

    let mut success_count = 0;
    let mut errors: Vec<String> = Vec::new();

    // Внутрішня рекурсивна функція для обходу
    fn process_dir(
        conn: &mut Connection,
        current_dir: &Path,
        storage_dir: &Path,
        user_id: &str,
        recursive: bool,
        success_count: &mut usize,
        errors: &mut Vec<String>,
    ) {
        let entries = match fs::read_dir(current_dir) {
            Ok(e) => e,
            Err(e) => {
                errors.push(format!("Не вдалося прочитати {}: {}", current_dir.display(), e));
                return;
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    errors.push(format!("Помилка читання запису: {}", e));
                    continue;
                }
            };
            let path = entry.path();

            // Пропускаємо приховані файли та .tmp
            if path.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with('.'))
                .unwrap_or(false)
            {
                continue;
            }

            if path.is_dir() {
                // Якщо рекурсивний режим — заходимо в підпапку
                if recursive {
                    process_dir(conn, &path, storage_dir, user_id, recursive, success_count, errors);
                }
                continue;
            }

            if !path.is_file() {
                continue;
            }

            // Пропускаємо .tmp файли
            if path.extension().is_some_and(|ext| ext == "tmp") {
                continue;
            }

            // Імпортуємо файл
            match process_ingest(conn, &path, storage_dir, user_id) {
                Ok(_) => *success_count += 1,
                Err(e) => errors.push(format!("{}: {}", path.display(), e)),
            }
        }
    }

    process_dir(conn, dir_path, storage_dir, user_id, recursive, &mut success_count, &mut errors);

    // Логуємо помилки, якщо вони були
    if !errors.is_empty() && success_count > 0 {
        eprintln!("Попередження: деякі файли не вдалося імпортувати:");
        for err in &errors {
            eprintln!("  - {}", err);
        }
    }

    Ok(success_count)
}
