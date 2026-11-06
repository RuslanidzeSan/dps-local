use crate::db::LOCAL_USER_ID;
use chrono::{NaiveDate, Utc};
use rusqlite::{params, Connection};
use uuid::Uuid;

/// Повна архівна картка документа (для панелі редагування).
#[derive(serde::Serialize)]
pub struct DocumentCard {
    pub id: String,
    pub display_name: String,
    pub file_kind: Option<String>,
    pub author: Option<String>,
    pub purpose: Option<String>,
    pub doc_date_from: Option<String>,
    pub doc_date_to: Option<String>,
    pub source_location: Option<String>,
    pub card_template: Option<String>,
    pub description: Option<String>,
}

/// Оновлення картки з фронтенду (пусті/NULL поля = очистити).
#[derive(serde::Deserialize, Default, Debug)]
pub struct CardUpdate {
    pub object_id: String,
    pub file_kind: Option<String>,
    pub author: Option<String>,
    pub purpose: Option<String>,
    pub doc_date_from: Option<String>,
    pub doc_date_to: Option<String>,
    pub source_location: Option<String>,
    pub card_template: Option<String>,
}

/// Допоміжна: чистий текст (обрізаний) або None.
fn clean_text(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

/// Допоміжна: дата у форматі YYYY-MM-DD або None; некоректна — помилка.
fn clean_date(v: Option<&str>) -> Result<Option<String>, String> {
    match v.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => match NaiveDate::parse_from_str(s, "%Y-%m-%d") {
            Ok(_) => Ok(Some(s.to_string())),
            Err(_) => Err(format!("Некоректна дата '{s}': очікується формат YYYY-MM-DD")),
        },
    }
}

pub fn get_document_card(conn: &Connection, object_id: &str) -> Result<DocumentCard, String> {
    conn.query_row(
        "SELECT o.display_name, o.file_kind, o.author, o.purpose, \
         o.doc_date_from, o.doc_date_to, o.source_location, o.description, o.card_template \
         FROM archive_objects o WHERE o.id = ?1",
        [object_id],
        |r| {
            Ok(DocumentCard {
                id: object_id.to_string(),
                display_name: r.get(0)?,
                file_kind: r.get(1)?,
                author: r.get(2)?,
                purpose: r.get(3)?,
                doc_date_from: r.get(4)?,
                doc_date_to: r.get(5)?,
                source_location: r.get(6)?,
                description: r.get(7)?,
                card_template: r.get(8)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => format!("Документ {object_id} не знайдено"),
        e => e.to_string(),
    })
}

pub fn update_document_card(conn: &Connection, u: &CardUpdate) -> Result<(), String> {
    let file_kind = clean_text(u.file_kind.clone());
    let author = clean_text(u.author.clone());
    let purpose = clean_text(u.purpose.clone());
    let doc_date_from = clean_date(u.doc_date_from.as_deref())?;
    let doc_date_to = clean_date(u.doc_date_to.as_deref())?;
    let source_location = clean_text(u.source_location.clone());
    let card_template = match clean_text(u.card_template.clone()) {
        None => None,
        Some(t) if ["standard", "photo", "document", "certificate"].contains(&t.as_str()) => Some(t),
        Some(t) => return Err(format!("Невідомий шаблон картки '{t}'")),
    };

    let updated_at = Utc::now().to_rfc3339();
    let n = conn.execute(
        "UPDATE archive_objects SET file_kind=?1, author=?2, purpose=?3, doc_date_from=?4, \
         doc_date_to=?5, source_location=?6, card_template=?9, updated_at=?7 WHERE id=?8",
        params![file_kind, author, purpose, doc_date_from, doc_date_to, source_location, updated_at, u.object_id, card_template],
    ).map_err(|e| e.to_string())?;
    if n == 0 {
        return Err(format!("Документ {} не знайдено", u.object_id));
    }

    conn.execute(
        "INSERT INTO audit_events(id,user_id,action,target_table,target_id,payload,created_at) \
         VALUES(?1,?2,'CARD_UPDATED','archive_objects',?3,?4,?5)",
        params![
            Uuid::new_v4().to_string(), LOCAL_USER_ID, u.object_id,
            "Оновлено архівну картку", Utc::now().to_rfc3339()
        ],
    ).map_err(|e| e.to_string())?;
    Ok(())
}
