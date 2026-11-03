use rusqlite::Connection;

pub struct AuditLogInfo {
    pub id: String,
    pub action_type: String,
    pub target_record_id: Option<String>,
    pub details: Option<String>,
    pub created_at: String,
}

pub fn get_audit_logs(conn: &Connection) -> Result<Vec<AuditLogInfo>, String> {
    let mut stmt = conn.prepare(
        "SELECT id, action, target_id, payload, created_at 
         FROM audit_events 
         ORDER BY created_at DESC 
         LIMIT 100"
    ).map_err(|e| e.to_string())?;

    let logs = stmt.query_map([], |row| {
        Ok(AuditLogInfo {
            id: row.get(0)?,
            action_type: row.get(1)?,
            target_record_id: row.get(2)?,
            details: row.get(3)?,
            created_at: row.get(4)?,
        })
    }).map_err(|e| e.to_string())?
      .collect::<Result<Vec<AuditLogInfo>, _>>()
      .map_err(|e| e.to_string())?;

    Ok(logs)
}
