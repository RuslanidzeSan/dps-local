use rusqlite::Connection;
use uuid::Uuid;
use chrono::{NaiveDate, Utc};
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

/// Атрибут обраного документа разом зі значенням (для картки, A3).
#[derive(serde::Serialize)]
pub struct ObjectAttrValue {
    pub attr_id: String,
    pub attr_name: String,
    pub data_type: String,
    pub unit: Option<String>,
    pub list_values: Option<String>,
    pub value: Option<String>,
}

pub fn get_object_attr_values(conn: &Connection, object_id: &str) -> Result<Vec<ObjectAttrValue>, String> {
    let mut stmt = conn.prepare(
        "SELECT a.id, a.name, a.data_type, a.unit, a.list_values, \
                v.value_text, v.value_number, v.value_date \
         FROM attr_definitions a \
         LEFT JOIN attr_values v ON v.attr_id = a.id AND v.object_id = ?1 \
         WHERE a.is_active = 1 ORDER BY a.name ASC",
    ).map_err(|e| e.to_string())?;
    let items = stmt.query_map([object_id], |row| {
        let data_type: String = row.get(2)?;
        let value_text: Option<String> = row.get(5)?;
        let value_number: Option<f64> = row.get(6)?;
        let value_date: Option<String> = row.get(7)?;
        let value = match data_type.as_str() {
            "number" => value_number.map(|n| if n.fract() == 0.0 { (n as i64).to_string() } else { n.to_string() }),
            "date" => value_date,
            _ => value_text,
        };
        Ok(ObjectAttrValue {
            attr_id: row.get(0)?,
            attr_name: row.get(1)?,
            data_type,
            unit: row.get(3)?,
            list_values: row.get(4)?,
            value,
        })
    }).map_err(|e| e.to_string())?
      .collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?;
    Ok(items)
}

pub fn set_object_attr_value(conn: &Connection, object_id: &str, attr_id: &str, value: Option<&str>) -> Result<(), String> {
    let exists: bool = conn.query_row("SELECT COUNT(*) > 0 FROM archive_objects WHERE id = ?1", [object_id], |r| r.get(0)).map_err(|e| e.to_string())?;
    if !exists { return Err(format!("Документ {object_id} не знайдено")); }

    let (dt, list_values): (String, Option<String>) = conn.query_row(
        "SELECT data_type, list_values FROM attr_definitions WHERE id = ?1 AND is_active = 1",
        [attr_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => format!("Атрибут {attr_id} не знайдено або вимкнено"),
        e => e.to_string(),
    })?;

    let clean = value.map(str::trim).filter(|s| !s.is_empty());

    match clean {
        None => {
            conn.execute("DELETE FROM attr_values WHERE object_id = ?1 AND attr_id = ?2",
                rusqlite::params![object_id, attr_id]).map_err(|e| e.to_string())?;
        }
        Some(v) => {
            let (vt, vn, vd): (Option<String>, Option<f64>, Option<String>) = match dt.as_str() {
                "number" => {
                    let n: f64 = v.parse().map_err(|_| format!("Значення «{v}» не є числом"))?;
                    (None, Some(n), None)
                }
                "date" => {
                    NaiveDate::parse_from_str(v, "%Y-%m-%d")
                        .map_err(|_| format!("Некоректна дата «{v}»: очікується YYYY-MM-DD"))?;
                    (None, None, Some(v.to_string()))
                }
                "select" => {
                    let allowed: Vec<&str> = list_values.as_deref().unwrap_or("").split(';').map(str::trim).collect();
                    if !allowed.contains(&v) {
                        return Err(format!("Значення «{v}» не входить у список: {}", allowed.join("; ")));
                    }
                    (Some(v.to_string()), None, None)
                }
                _ => (Some(v.to_string()), None, None),
            };
            conn.execute(
                "INSERT INTO attr_values (id, object_id, attr_id, value_text, value_number, value_date, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
                 ON CONFLICT(object_id, attr_id) DO UPDATE SET value_text = ?4, value_number = ?5, value_date = ?6",
                rusqlite::params![Uuid::new_v4().to_string(), object_id, attr_id, vt, vn, vd, Utc::now().to_rfc3339()],
            ).map_err(|e| e.to_string())?;
        }
    }

    let payload = match clean {
        Some(v) => format!("attr={attr_id} value={v}"),
        None => format!("attr={attr_id} очищено"),
    };
    audit::record_event(conn, LOCAL_USER_ID, "ATTR_VALUE_SET", "attr_values", object_id, Some(&payload)).map_err(|e| e.to_string())?;
    Ok(())
}
