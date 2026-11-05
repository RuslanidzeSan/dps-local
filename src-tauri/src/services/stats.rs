use rusqlite::Connection;

pub struct RegistryStats {
    pub total_objects: i64,
    pub active: i64,
    pub pending_deletion: i64,
    pub checksum_error: i64,
    pub other_status: i64,
    pub total_bytes: i64,
    pub inaccessible_locations: i64,
}

pub fn get_registry_stats(conn: &Connection) -> Result<RegistryStats, String> {
    let (total, active, pending, damaged) = conn.query_row(
        "SELECT COUNT(*),
                COALESCE(SUM(CASE WHEN status = 'Active' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status = 'Pending_Deletion' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status = 'Checksum_Error' THEN 1 ELSE 0 END), 0)
         FROM archive_objects",
        [],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?)),
    ).map_err(|e| e.to_string())?;

    let total_bytes: i64 = conn.query_row(
        "SELECT COALESCE(SUM(file_size_bytes), 0) FROM storage_locations",
        [], |r| r.get(0)).map_err(|e| e.to_string())?;
    let inaccessible: i64 = conn.query_row(
        "SELECT COUNT(*) FROM storage_locations WHERE is_accessible = 0",
        [], |r| r.get(0)).map_err(|e| e.to_string())?;

    Ok(RegistryStats {
        total_objects: total,
        active,
        pending_deletion: pending,
        checksum_error: damaged,
        other_status: total - active - pending - damaged,
        total_bytes,
        inaccessible_locations: inaccessible,
    })
}
