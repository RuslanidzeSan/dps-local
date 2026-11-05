use rusqlite::Connection;
use uuid::Uuid;
use chrono::Utc;
use crate::db::{audit, LOCAL_USER_ID};

pub struct AttrInfo {
    pub id: String,
    pub code: String,
    pub name: String,
    pub data_type: String,
    pub unit: Option<String>,
    pub list_values: Option<String>,
    pub created_at: String,
    pub usage_count: i64,
}

fn slugify(name: &str) -> String {
    let mut slug: String = name.trim().to_lowercase()
        .chars().map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    while slug.contains("__") { slug = slug.replace("__", "_"); }
    let trimmed = slug.trim_matches('_').to_string();
    if trimmed.is_empty() { format!("attr_{}", Uuid::new_v4().simple()) } else { trimmed }
}

pub fn get_all_attrs_with_stats(conn: &Connection) -> Result<Vec<AttrInfo>, String> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.code, a.name, a.data_type, a.unit, a.list_values, a.created_at,
                (SELECT COUNT(*) FROM attr_values v WHERE v.attr_id = a.id) as usage_count
         FROM attr_definitions a WHERE a.is_active = 1
         ORDER BY usage_count DESC, a.name ASC"
    ).map_err(|e| e.to_string())?;
    let items = stmt.query_map([], |row| {
        Ok(AttrInfo {
            id: row.get(0)?,
            code: row.get(1)?,
            name: row.get(2)?,
            data_type: row.get(3)?,
            unit: row.get(4)?,
            list_values: row.get(5)?,
            created_at: row.get(6)?,
            usage_count: row.get(7)?,
        })
    }).map_err(|e| e.to_string())?
      .collect::<Result<Vec<AttrInfo>, _>>()
      .map_err(|e| e.to_string())?;
    Ok(items)
}

pub fn add_attr(conn: &mut Connection, name: &str, data_type: &str, unit: Option<&str>, list_values: Option<&str>) -> Result<(), String> {
    let clean_name = name.trim().to_string();
    if clean_name.is_empty() {
        return Err("Назва атрибута не може бути порожньою".to_string());
    }
    let dt = data_type.trim().to_lowercase();
    if !["text", "number", "date", "select"].contains(&dt.as_str()) {
        return Err("Невідомий тип атрибута (дозволені: text, number, date, select)".to_string());
    }
    let norm_list: Option<String> = match list_values {
        Some(s) => {
            let items: Vec<String> = s.split(';').map(str::trim).filter(|p| !p.is_empty()).map(String::from).collect();
            if items.is_empty() { None } else { Some(items.join(";")) }
        }
        None => None,
    };
    if dt == "select" && norm_list.is_none() {
        return Err("Для типу «Список» потрібно задати хоча б одне значення через «;»".to_string());
    }
    let unit_v = unit.map(str::trim).filter(|s| !s.is_empty());

    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let new_id = Uuid::new_v4().to_string();
    let code = slugify(&clean_name);
    let now = Utc::now().to_rfc3339();
    tx.execute(
        "INSERT INTO attr_definitions (id, code, name, data_type, unit, list_values, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        (&new_id, &code, &clean_name, &dt, unit_v, norm_list, &now),
    ).map_err(|e| e.to_string())?;

    let audit_id = Uuid::new_v4().to_string();
    let _ = tx.execute(
        "INSERT INTO audit_events (id, user_id, action, target_table, target_id, payload, created_at) VALUES (?1, '00000000-0000-4000-8000-000000000001', 'METADATA_UPDATE', 'attr_definitions', ?2, ?3, ?4)",
        (&audit_id, &new_id, format!("Створено атрибут: {}", clean_name), &now),
    );

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn update_attr(conn: &Connection, attr_id: &str, new_name: &str) -> Result<(), String> {
    let clean_name = new_name.trim().to_string();
    if clean_name.is_empty() {
        return Err("Назва атрибута не може бути порожньою".to_string());
    }
    conn.execute(
        "UPDATE attr_definitions SET name = ?1 WHERE id = ?2",
        (&clean_name, attr_id),
    ).map_err(|e| e.to_string())?;
    audit::record_event(conn, LOCAL_USER_ID, "ATTR_RENAMED", "attr_definitions", attr_id, Some(&clean_name)).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_attr(conn: &Connection, attr_id: &str) -> Result<(), String> {
    conn.execute("UPDATE attr_definitions SET is_active = 0 WHERE id = ?1", [attr_id]).map_err(|e| e.to_string())?;
    audit::record_event(conn, LOCAL_USER_ID, "ATTR_DEACTIVATED", "attr_definitions", attr_id, None).map_err(|e| e.to_string())?;
    Ok(())
}
