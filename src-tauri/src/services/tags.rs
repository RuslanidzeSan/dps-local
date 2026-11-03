use rusqlite::Connection;
use uuid::Uuid;
use chrono::Utc;
use crate::db::{audit, LOCAL_USER_ID};

pub struct TagInfo {
    pub id: String,
    pub tag_name: String,
    pub created_at: String,
    pub usage_count: i64,
}

pub fn add_tag_to_object(conn: &mut Connection, object_id: &str, name: &str) -> Result<(), String> {
    let clean_name = name.trim().to_lowercase();
    if clean_name.is_empty() {
        return Err("Назва тегу не може бути порожньою".to_string());
    }

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

    let tag_id: String = match tx.query_row(
        "SELECT id FROM ref_tags WHERE tag_name = ?1",
        [&clean_name],
        |row| row.get(0),
    ) {
        Ok(id) => id,
        Err(_) => {
            let new_id = Uuid::new_v4().to_string();
            let now = Utc::now().to_rfc3339();
            tx.execute(
                "INSERT INTO ref_tags (id, tag_name, created_at) VALUES (?1, ?2, ?3)",
                (&new_id, &clean_name, &now),
            ).map_err(|e| e.to_string())?;
            new_id
        }
    };

    tx.execute(
        "INSERT OR IGNORE INTO map_file_tags (file_id, tag_id) VALUES (?1, ?2)",
        (object_id, &tag_id),
    ).map_err(|e| e.to_string())?;

    let audit_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let _ = tx.execute(
        "INSERT INTO audit_events (id, user_id, action, target_table, target_id, payload, created_at) VALUES (?1, '00000000-0000-4000-8000-000000000001', ?2, 'archive_objects', ?3, ?4, ?5)",
        (&audit_id, "METADATA_UPDATE", object_id, format!("Додано тег: {}", clean_name), &now),
    );

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_object_tags(conn: &Connection, object_id: &str) -> Result<Vec<String>, String> {
    let mut stmt = conn.prepare(
        "SELECT t.tag_name FROM ref_tags t JOIN map_file_tags m ON m.tag_id = t.id WHERE m.file_id = ?1"
    ).map_err(|e| e.to_string())?;
    let items = stmt.query_map([object_id], |row| row.get(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<String>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(items)
}

pub fn get_all_tags_with_stats(conn: &Connection) -> Result<Vec<TagInfo>, String> {
    let mut stmt = conn.prepare(
        "SELECT t.id, t.tag_name, t.created_at, 
                (SELECT COUNT(*) FROM map_file_tags m WHERE m.tag_id = t.id) as usage_count
         FROM ref_tags t WHERE t.is_active = 1
         ORDER BY usage_count DESC, t.tag_name ASC"
    ).map_err(|e| e.to_string())?;

    let tags = stmt.query_map([], |row| {
        Ok(TagInfo {
            id: row.get(0)?,
            tag_name: row.get(1)?,
            created_at: row.get(2)?,
            usage_count: row.get(3)?,
        })
    }).map_err(|e| e.to_string())?
      .collect::<Result<Vec<TagInfo>, _>>()
      .map_err(|e| e.to_string())?;

    Ok(tags)
}

pub fn update_tag(conn: &Connection, tag_id: &str, new_name: &str) -> Result<(), String> {
    let clean_name = new_name.trim().to_lowercase();
    if clean_name.is_empty() {
        return Err("Назва тегу не може бути порожньою".to_string());
    }
    conn.execute(
        "UPDATE ref_tags SET tag_name = ?1 WHERE id = ?2",
        (&clean_name, tag_id),
    ).map_err(|e| e.to_string())?;
    audit::record_event(conn, LOCAL_USER_ID, "TAG_RENAMED", "ref_tags", tag_id, Some(&clean_name)).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_tag(conn: &Connection, tag_id: &str) -> Result<(), String> {
    conn.execute("UPDATE ref_tags SET is_active = 0 WHERE id = ?1", [tag_id]).map_err(|e| e.to_string())?;
    audit::record_event(conn, LOCAL_USER_ID, "TAG_DEACTIVATED", "ref_tags", tag_id, None).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn merge_tags(conn: &mut Connection, source_tag_id: &str, target_tag_id: &str) -> Result<(), String> {
    if source_tag_id == target_tag_id {
        return Err("Неможливо злити тег із самим собою".to_string());
    }

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;

    tx.execute(
        "UPDATE OR IGNORE map_file_tags SET tag_id = ?1 WHERE tag_id = ?2",
        (target_tag_id, source_tag_id),
    ).map_err(|e| e.to_string())?;

    tx.execute("UPDATE ref_tags SET is_active = 0 WHERE id = ?1", [source_tag_id]).map_err(|e| e.to_string())?;

    let audit_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let _ = tx.execute(
        "INSERT INTO audit_events (id, user_id, action, target_table, target_id, payload, created_at) VALUES (?1, '00000000-0000-4000-8000-000000000001', ?2, 'ref_tags', ?3, ?4, ?5)",
        (&audit_id, "METADATA_UPDATE", target_tag_id, format!("Злиття тегів: джерело {} в ціль {}", source_tag_id, target_tag_id), &now),
    );

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}
