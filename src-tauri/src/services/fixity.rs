use crate::db::LOCAL_USER_ID;
use chrono::Utc;
use rusqlite::{params, Connection};
use std::path::Path;
use uuid::Uuid;
use super::ingest::sha256_file;

/// Одна проблема цілісності, знайдена під час перевірки.
#[derive(serde::Serialize)]
pub struct FixityProblem {
    pub object_id: String,
    pub display_name: String,
    pub file_path: String,
    /// "mismatch" = сума не збігається; "missing" = файл зник; "unreadable" = не вдалося прочитати
    pub status: String,
    pub expected_sha256: String,
    pub actual_sha256: String,
}

/// Підсумковий звіт перевірки всього архіву.
#[derive(serde::Serialize)]
pub struct FixityReport {
    pub checked: usize,
    pub ok: usize,
    pub problems: Vec<FixityProblem>,
}

/// Перераховує SHA-256 для всіх managed-файлів архіву і порівнює
/// зі збереженими сумами. Reference-посилання (без суми) пропускає.
pub fn verify_archive_fixity(conn: &Connection) -> Result<FixityReport, String> {
    // Вибираємо всі managed-локації, у яких є контрольна сума
    let rows = {
        let mut stmt = conn.prepare(
            "SELECT o.id, o.display_name, s.file_path, s.checksum_sha256 \
             FROM storage_locations s \
             JOIN representations r ON r.id = s.representation_id \
             JOIN archive_objects o ON o.id = r.object_id \
             WHERE s.storage_type = 'managed' AND s.checksum_sha256 IS NOT NULL",
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        }).map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
    };

    let mut report = FixityReport { checked: 0, ok: 0, problems: Vec::new() };
    for (object_id, display_name, file_path, expected) in rows {
        report.checked += 1;
        let path = Path::new(&file_path);
        if !path.is_file() {
            report.problems.push(FixityProblem {
                object_id, display_name, file_path,
                status: "missing".into(),
                expected_sha256: expected, actual_sha256: String::new(),
            });
            continue;
        }
        match sha256_file(path) {
            Ok(actual) if actual == expected => report.ok += 1,
            Ok(actual) => report.problems.push(FixityProblem {
                object_id, display_name, file_path,
                status: "mismatch".into(),
                expected_sha256: expected, actual_sha256: actual,
            }),
            Err(e) => report.problems.push(FixityProblem {
                object_id, display_name, file_path,
                status: "unreadable".into(),
                expected_sha256: expected,
                actual_sha256: e,
            }),
        }
    }

    // Підсумковий запис у аудит (незмінний журнал)
    let payload = serde_json::json!({
        "checked": report.checked,
        "ok": report.ok,
        "problems": report.problems.len(),
    }).to_string();
    conn.execute(
        "INSERT INTO audit_events(id,user_id,action,target_table,target_id,payload,created_at) \
         VALUES(?1,?2,'FIXITY_AUDIT_COMPLETED','archive_objects','*',?3,?4)",
        params![Uuid::new_v4().to_string(), LOCAL_USER_ID, payload, Utc::now().to_rfc3339()],
    ).map_err(|e| e.to_string())?;

    Ok(report)
}
