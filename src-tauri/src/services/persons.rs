use rusqlite::Connection;
use uuid::Uuid;
use chrono::Utc;
use crate::db::{audit, LOCAL_USER_ID};

pub struct PersonInfo {
    pub id: String,
    pub person_name: String,
    pub created_at: String,
    pub usage_count: i64,
}

pub fn add_person_to_object(conn: &mut Connection, object_id: &str, name: &str) -> Result<(), String> {
    let clean_name = name.trim().to_lowercase();
    if clean_name.is_empty() {
        return Err("Ім'я не може бути порожнім".to_string());
    }

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

    let person_id: String = match tx.query_row(
        "SELECT id FROM ref_persons WHERE person_name = ?1",
        [&clean_name],
        |row| row.get(0),
    ) {
        Ok(id) => id,
        Err(_) => {
            let new_id = Uuid::new_v4().to_string();
            let now = Utc::now().to_rfc3339();
            tx.execute(
                "INSERT INTO ref_persons (id, person_name, created_at) VALUES (?1, ?2, ?3)",
                (&new_id, &clean_name, &now),
            ).map_err(|e| e.to_string())?;
            new_id
        }
    };

    tx.execute(
        "INSERT OR IGNORE INTO map_file_persons (file_id, person_id) VALUES (?1, ?2)",
        (object_id, &person_id),
    ).map_err(|e| e.to_string())?;

    let audit_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let _ = tx.execute(
        "INSERT INTO audit_events (id, user_id, action, target_table, target_id, payload, created_at) VALUES (?1, '00000000-0000-4000-8000-000000000001', ?2, 'archive_objects', ?3, ?4, ?5)",
        (&audit_id, "METADATA_UPDATE", object_id, format!("Додано особу: {}", clean_name), &now),
    );

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_object_persons(conn: &Connection, object_id: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn.prepare(
        "SELECT t.person_name FROM ref_persons t JOIN map_file_persons m ON m.person_id = t.id WHERE m.file_id = ?1"
    ).map_err(|e| e.to_string())?;
    let items = stmt.query_map([object_id], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(items)
}

pub fn get_all_persons_with_stats(conn: &Connection) -> Result<Vec<PersonInfo>, String> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.person_name, t.created_at, 
                (SELECT COUNT(*) FROM map_file_persons m WHERE m.person_id = t.id) as usage_count
         FROM ref_persons t WHERE t.is_active = 1
         ORDER BY usage_count DESC, t.person_name ASC"
    ).map_err(|e| e.to_string())?;

    let persons = stmt.query_map([], |row| {
        Ok(PersonInfo {
            id: row.get(0)?,
            person_name: row.get(1)?,
            created_at: row.get(2)?,
            usage_count: row.get(3)?,
        })
    }).map_err(|e| e.to_string())?
      .collect::<Result<Vec<PersonInfo>, _>>()
      .map_err(|e| e.to_string())?;

    Ok(persons)
}

pub fn update_person(conn: &Connection, person_id: &str, new_name: &str) -> Result<(), String> {
    let clean_name = new_name.trim().to_lowercase();
    if clean_name.is_empty() {
        return Err("Ім'я не може бути порожнім".to_string());
    }
    conn.execute(
        "UPDATE ref_persons SET person_name = ?1 WHERE id = ?2",
        (&clean_name, person_id),
    ).map_err(|e| e.to_string())?;
    audit::record_event(conn, LOCAL_USER_ID, "PERSON_RENAMED", "ref_persons", person_id, Some(&clean_name)).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_person(conn: &Connection, person_id: &str) -> Result<(), String> {
    conn.execute("UPDATE ref_persons SET is_active = 0 WHERE id = ?1", [person_id]).map_err(|e| e.to_string())?;
    audit::record_event(conn, LOCAL_USER_ID, "PERSON_DEACTIVATED", "ref_persons", person_id, None).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn merge_persons(conn: &mut Connection, source_id: &str, target_id: &str) -> Result<(), String> {
    if source_id == target_id {
        return Err("Неможливо злити особу з самою собою".to_string());
    }

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

    tx.execute(
        "UPDATE OR IGNORE map_file_persons SET person_id = ?1 WHERE person_id = ?2",
        (target_id, source_id),
    ).map_err(|e| e.to_string())?;

    tx.execute("UPDATE ref_persons SET is_active = 0 WHERE id = ?1", [source_id]).map_err(|e| e.to_string())?;

    let audit_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let _ = tx.execute(
        "INSERT INTO audit_events (id, user_id, action, target_table, target_id, payload, created_at) VALUES (?1, '00000000-0000-4000-8000-000000000001', ?2, 'ref_persons', ?3, ?4, ?5)",
        (&audit_id, "METADATA_UPDATE", target_id, format!("Злиття персоналій: джерело {} в ціль {}", source_id, target_id), &now),
    );

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}
