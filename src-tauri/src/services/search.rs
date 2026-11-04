use rusqlite::{Connection, ToSql};
use super::ingest::ArchiveObjectView;

/// Умови пошуку реєстру. Поля-Option, що дорівнюють None/порожнім,
/// фільтр не застосовують. dates у форматі YYYY-MM-DD, розміри в байтах.
#[derive(serde::Deserialize, Default, Debug)]
pub struct SearchFilters {
    pub query: Option<String>,
    pub tag_id: Option<String>,
    pub person_id: Option<String>,
    pub location_id: Option<String>,
    pub extension: Option<String>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub doc_date_from: Option<String>,
    pub doc_date_to: Option<String>,
    pub min_size: Option<i64>,
    pub max_size: Option<i64>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

const DEFAULT_PAGE_SIZE: i64 = 50;
const MAX_PAGE_SIZE: i64 = 200;

/// Будує частину " AND ..." для WHERE з усіма фільтрами.
/// Використовується і списком, і підрахунком — щоб умови ніколи не розійшлися.
fn build_where(f: &SearchFilters) -> (String, Vec<Box<dyn ToSql>>) {
    let mut sql = String::new();
    let mut args: Vec<Box<dyn ToSql>> = vec![];

    if let Some(q) = f.query.as_deref().filter(|x| !x.trim().is_empty()) {
        sql.push_str(" AND (o.display_name LIKE ? OR o.original_name LIKE ? OR o.description LIKE ? OR o.file_extension LIKE ?)");
        let p = format!("%{}%", q.trim());
        args.push(Box::new(p.clone()));
        args.push(Box::new(p.clone()));
        args.push(Box::new(p.clone()));
        args.push(Box::new(p));
    }
    for (value, column) in [
        (&f.tag_id, "mt.tag_id"),
        (&f.person_id, "mp.person_id"),
        (&f.location_id, "ml.location_id"),
    ] {
        if let Some(v) = value.as_deref().filter(|x| !x.is_empty()) {
            sql.push_str(&format!(" AND {column}=?"));
            args.push(Box::new(v.to_string()));
        }
    }
    if let Some(ext) = f.extension.as_deref().filter(|x| !x.trim().is_empty()) {
        sql.push_str(" AND o.file_extension=?");
        args.push(Box::new(ext.trim().to_ascii_lowercase()));
    }
    if let Some(d) = f.date_from.as_deref().filter(|x| !x.trim().is_empty()) {
        // created_at — RFC3339, порівнюємо лише дату (перші 10 символів)
        sql.push_str(" AND substr(o.created_at,1,10)>=?");
        args.push(Box::new(d.trim().to_string()));
    }
    if let Some(d) = f.date_to.as_deref().filter(|x| !x.trim().is_empty()) {
        sql.push_str(" AND substr(o.created_at,1,10)<=?");
        args.push(Box::new(d.trim().to_string()));
    }
    // Дата Документу: перетин діапазонів. Документ з датами [from,to]
    // потрапляє у запит, якщо його діапазон перекриває фільтр.
    // Документи без дат при активному фільтрі не показуються (NULL-порівняння).
    if let Some(d) = f.doc_date_from.as_deref().filter(|x| !x.trim().is_empty()) {
        sql.push_str(" AND COALESCE(o.doc_date_to, o.doc_date_from) >= ?");
        args.push(Box::new(d.trim().to_string()));
    }
    if let Some(d) = f.doc_date_to.as_deref().filter(|x| !x.trim().is_empty()) {
        sql.push_str(" AND COALESCE(o.doc_date_from, o.doc_date_to) <= ?");
        args.push(Box::new(d.trim().to_string()));
    }
    if let Some(n) = f.min_size.filter(|x| *x >= 0) {
        sql.push_str(" AND s.file_size_bytes>=?");
        args.push(Box::new(n));
    }
    if let Some(n) = f.max_size.filter(|x| *x >= 0) {
        sql.push_str(" AND s.file_size_bytes<=?");
        args.push(Box::new(n));
    }
    (sql, args)
}

fn page_params(f: &SearchFilters) -> (i64, i64) {
    let limit = match f.limit {
        Some(n) if n > 0 => n.min(MAX_PAGE_SIZE),
        _ => DEFAULT_PAGE_SIZE,
    };
    let offset = f.offset.filter(|x| *x >= 0).unwrap_or(0);
    (limit, offset)
}

pub fn search_archive_objects_filtered(
    conn: &Connection,
    f: &SearchFilters,
) -> Result<Vec<ArchiveObjectView>, String> {
    let (where_sql, args) = build_where(f);
    let (limit, offset) = page_params(f);

    let sql = format!(
        "SELECT DISTINCT o.id,o.display_name,o.status,o.description,s.file_path,s.file_size_bytes,\
         COALESCE(s.checksum_sha256,''),s.storage_type,s.is_accessible,o.created_at \
         FROM archive_objects o \
         JOIN representations r ON r.object_id=o.id \
         JOIN storage_locations s ON s.representation_id=r.id \
         LEFT JOIN map_file_tags mt ON mt.file_id=o.id \
         LEFT JOIN map_file_persons mp ON mp.file_id=o.id \
         LEFT JOIN map_file_locations ml ON ml.file_id=o.id \
         WHERE o.status!='Destroyed'{where_sql} \
         ORDER BY o.created_at DESC LIMIT {limit} OFFSET {offset}"
    );

    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    stmt.query_map(rusqlite::params_from_iter(args), |r| {
        Ok(ArchiveObjectView {
            id: r.get(0)?,
            display_name: r.get(1)?,
            status: r.get(2)?,
            description: r.get(3)?,
            file_path: r.get(4)?,
            file_size_bytes: r.get(5)?,
            checksum_sha256: r.get(6)?,
            storage_type: r.get(7)?,
            is_accessible: r.get::<_, i64>(8)? != 0,
            created_at: r.get(9)?,
        })
    })
    .map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())
}

/// Загальна кільсть документів, що відповідають фільтрам (без пагінації).
/// Фронтенд використовує її, щоб порахувати кількість сторінок.
pub fn count_archive_objects_filtered(conn: &Connection, f: &SearchFilters) -> Result<i64, String> {
    let (where_sql, args) = build_where(f);
    let sql = format!(
        "SELECT COUNT(DISTINCT o.id) \
         FROM archive_objects o \
         JOIN representations r ON r.object_id=o.id \
         JOIN storage_locations s ON s.representation_id=r.id \
         LEFT JOIN map_file_tags mt ON mt.file_id=o.id \
         LEFT JOIN map_file_persons mp ON mp.file_id=o.id \
         LEFT JOIN map_file_locations ml ON ml.file_id=o.id \
         WHERE o.status!='Destroyed'{where_sql}"
    );
    let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
    stmt.query_row(rusqlite::params_from_iter(args), |r| r.get(0))
        .map_err(|e| e.to_string())
}
