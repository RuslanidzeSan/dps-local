pub mod db;
pub mod services;

use tauri::AppHandle;
use std::path::PathBuf;

#[tauri::command]
fn greet(name: &str) -> Result<String, String> {
    Ok(format!("Hello, {}! You've been greeted from Rust.", name))
}

#[tauri::command]
async fn ingest_file(_app: AppHandle, source_path: String) -> Result<services::ingest::IngestResult, String> {
    let mut conn = db::open_database()?;
    let storage_dir = db::get_storage_dir();
    let path = PathBuf::from(source_path);

    services::ingest::process_ingest(&mut conn, &path, &storage_dir, db::LOCAL_USER_ID)
}

#[tauri::command]
fn create_backup_command(backup_dir: String) -> Result<String, String> {
    let path = services::backup::create_backup_default(std::path::Path::new(&backup_dir))?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
fn restore_backup_command(backup_path: String) -> Result<(), String> {
    services::backup::restore_backup_at(
        std::path::Path::new(&backup_path),
        &db::get_database_path(),
        &db::get_storage_dir(),
    )
}

#[tauri::command]
async fn ingest_directory_command(
    _app: AppHandle,
    dir_path: String,
    recursive: bool,
) -> Result<usize, String> {
    let mut conn = db::open_database()?;
    let storage_dir = db::get_storage_dir();
    let path = PathBuf::from(dir_path);

    services::ingest::ingest_directory(&mut conn, &path, &storage_dir, db::LOCAL_USER_ID, recursive)
}

#[tauri::command]
async fn export_objects_csv_command() -> Result<String, String> {
    let conn = db::open_database()?;
    services::export::export_objects_csv(&conn)
}

#[tauri::command]
async fn export_objects_json_command() -> Result<String, String> {
    let conn = db::open_database()?;
    services::export::export_objects_json(&conn)
}

#[tauri::command]
fn save_export_command(kind: String) -> Result<String, String> {
    let conn = db::open_database()?;
    let content = match kind.as_str() {
        "csv" => services::export::export_objects_csv(&conn)?,
        "json" => services::export::export_objects_json(&conn)?,
        other => return Err(format!("Невідомий тип експорту: {other}")),
    };
    let path = services::export::save_export_at(&db::get_exports_dir(), &kind, &content)?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
fn list_exports_command() -> Result<Vec<services::export::ExportInfoDTO>, String> {
    services::export::list_exports_at(&db::get_exports_dir())
}

#[tauri::command]
fn rename_export_command(old_name: String, new_name: String) -> Result<(), String> {
    services::export::rename_export_at(&db::get_exports_dir(), &old_name, &new_name)
}

#[tauri::command]
async fn index_reference_file(_app: AppHandle, source_path: String) -> Result<services::ingest::IngestResult, String> {
    let mut conn = db::open_database()?;
    services::ingest::index_reference(&mut conn, &PathBuf::from(source_path), db::LOCAL_USER_ID)
}

#[tauri::command]
async fn reconcile_reference_locations(_app: AppHandle) -> Result<usize, String> {
    let mut conn = db::open_database()?;
    services::ingest::reconcile_reference_locations(&mut conn)
}

#[tauri::command]
async fn get_archive_objects(_app: AppHandle) -> Result<Vec<services::ingest::ArchiveObjectView>, String> {
    let conn = db::open_database()?;
    services::ingest::search_archive_objects(&conn, None, None, None, None)
}

#[tauri::command]
async fn export_archive_csv(_app: AppHandle, extension: Option<String>) -> Result<String, String> {
    let conn = db::open_database()?;
    services::export::export_objects_csv_filtered(&conn, extension.as_deref())
}

#[tauri::command]
async fn export_archive_json(_app: AppHandle, extension: Option<String>) -> Result<String, String> {
    let conn = db::open_database()?;
    services::export::export_objects_json_filtered(&conn, extension.as_deref())
}

// --- ТЕГИ ---
#[tauri::command]
async fn get_object_tags(_app: AppHandle, object_id: String) -> Result<Vec<String>, String> {
    let conn = db::open_database()?;
    services::tags::get_object_tags(&conn, &object_id)
}

#[tauri::command]
async fn add_tag_to_object(_app: AppHandle, object_id: String, tag_name: String) -> Result<(), String> {
    let mut conn = db::open_database()?;
    services::tags::add_tag_to_object(&mut conn, &object_id, &tag_name)
}

#[derive(serde::Serialize)]
pub struct TagInfoDTO {
    id: String,
    tag_name: String,
    created_at: String,
    usage_count: i64,
}

#[tauri::command]
async fn get_all_tags_modal(_app: AppHandle) -> Result<Vec<TagInfoDTO>, String> {
    let conn = db::open_database()?;
    let tags = services::tags::get_all_tags_with_stats(&conn)?;
    Ok(tags.into_iter().map(|t| TagInfoDTO {
        id: t.id,
        tag_name: t.tag_name,
        created_at: t.created_at,
        usage_count: t.usage_count,
    }).collect())
}

#[tauri::command]
async fn update_tag_name(_app: AppHandle, tag_id: String, new_name: String) -> Result<(), String> {
    let conn = db::open_database()?;
    services::tags::update_tag(&conn, &tag_id, &new_name)
}

#[tauri::command]
async fn delete_tag_record(_app: AppHandle, tag_id: String) -> Result<(), String> {
    let conn = db::open_database()?;
    services::tags::delete_tag(&conn, &tag_id)
}

#[tauri::command]
async fn merge_tags_handler(_app: AppHandle, source_tag_id: String, target_tag_id: String) -> Result<(), String> {
    let mut conn = db::open_database()?;
    services::tags::merge_tags(&mut conn, &source_tag_id, &target_tag_id)
}

// --- ПЕРСОНАЛІЇ ---
#[tauri::command]
async fn get_object_persons(_app: AppHandle, object_id: String) -> Result<Vec<String>, String> {
    let conn = db::open_database()?;
    services::persons::get_object_persons(&conn, &object_id)
}

#[tauri::command]
async fn add_person_to_object(_app: AppHandle, object_id: String, tag_name: String) -> Result<(), String> {
    let mut conn = db::open_database()?;
    services::persons::add_person_to_object(&mut conn, &object_id, &tag_name)
}

#[derive(serde::Serialize)]
pub struct PersonInfoDTO {
    id: String,
    person_name: String,
    created_at: String,
    usage_count: i64,
}

#[tauri::command]
async fn get_all_persons_modal(_app: AppHandle) -> Result<Vec<PersonInfoDTO>, String> {
    let conn = db::open_database()?;
    let persons = services::persons::get_all_persons_with_stats(&conn)?;
    Ok(persons.into_iter().map(|p| PersonInfoDTO {
        id: p.id,
        person_name: p.person_name,
        created_at: p.created_at,
        usage_count: p.usage_count,
    }).collect())
}

#[tauri::command]
async fn update_person_name(_app: AppHandle, person_id: String, new_name: String) -> Result<(), String> {
    let conn = db::open_database()?;
    services::persons::update_person(&conn, &person_id, &new_name)
}

#[tauri::command]
async fn delete_person_record(_app: AppHandle, person_id: String) -> Result<(), String> {
    let conn = db::open_database()?;
    services::persons::delete_person(&conn, &person_id)
}

#[tauri::command]
async fn merge_persons_handler(_app: AppHandle, source_id: String, target_id: String) -> Result<(), String> {
    let mut conn = db::open_database()?;
    services::persons::merge_persons(&mut conn, &source_id, &target_id)
}

// --- ЛОКАЦІЇ ---
#[tauri::command]
async fn get_object_locations(_app: AppHandle, object_id: String) -> Result<Vec<String>, String> {
    let conn = db::open_database()?;
    services::locations::get_object_locations(&conn, &object_id)
}

#[tauri::command]
async fn add_location_to_object(_app: AppHandle, object_id: String, tag_name: String) -> Result<(), String> {
    let mut conn = db::open_database()?;
    services::locations::add_location_to_object(&mut conn, &object_id, &tag_name)
}

#[derive(serde::Serialize)]
pub struct LocationInfoDTO {
    id: String,
    location_name: String,
    created_at: String,
    usage_count: i64,
}

#[tauri::command]
async fn get_all_locations_modal(_app: AppHandle) -> Result<Vec<LocationInfoDTO>, String> {
    let conn = db::open_database()?;
    let locations = services::locations::get_all_locations_with_stats(&conn)?;
    Ok(locations.into_iter().map(|l| LocationInfoDTO {
        id: l.id,
        location_name: l.location_name,
        created_at: l.created_at,
        usage_count: l.usage_count,
    }).collect())
}

#[tauri::command]
async fn update_location_name(_app: AppHandle, location_id: String, new_name: String) -> Result<(), String> {
    let conn = db::open_database()?;
    services::locations::update_location(&conn, &location_id, &new_name)
}

#[tauri::command]
async fn delete_location_record(_app: AppHandle, location_id: String) -> Result<(), String> {
    let conn = db::open_database()?;
    services::locations::delete_location(&conn, &location_id)
}

#[tauri::command]
async fn merge_locations_handler(_app: AppHandle, source_id: String, target_id: String) -> Result<(), String> {
    let mut conn = db::open_database()?;
    services::locations::merge_locations(&mut conn, &source_id, &target_id)
}

// --- АУДИТ ---
#[derive(serde::Serialize)]
pub struct AuditLogDTO {
    id: String,
    action_type: String,
    target_record_id: Option<String>,
    details: Option<String>,
    created_at: String,
}

#[tauri::command]
async fn get_audit_logs_modal(_app: AppHandle) -> Result<Vec<AuditLogDTO>, String> {
    let conn = db::open_database()?;
    let logs = services::audit::get_audit_logs(&conn)?;
    Ok(logs.into_iter().map(|l| AuditLogDTO {
        id: l.id,
        action_type: l.action_type,
        target_record_id: l.target_record_id,
        details: l.details,
        created_at: l.created_at,
    }).collect())
}

#[tauri::command]
async fn search_objects_filtered(
    _app: AppHandle,
    query: Option<String>,
    tag_id: Option<String>,
    person_id: Option<String>,
    location_id: Option<String>,
) -> Result<Vec<services::ingest::ArchiveObjectView>, String> {
    let conn = db::open_database()?;
    services::ingest::search_archive_objects(
        &conn,
        query.as_deref(),
        tag_id.as_deref(),
        person_id.as_deref(),
        location_id.as_deref(),
    )
}

#[tauri::command]
async fn search_objects_v2(
    _app: AppHandle,
    filters: services::search::SearchFilters,
) -> Result<Vec<services::ingest::ArchiveObjectView>, String> {
    let conn = db::open_database()?;
    services::search::search_archive_objects_filtered(&conn, &filters)
}

#[tauri::command]
async fn count_objects_command(
    _app: AppHandle,
    filters: services::search::SearchFilters,
) -> Result<i64, String> {
    let conn = db::open_database()?;
    services::search::count_archive_objects_filtered(&conn, &filters)
}

#[tauri::command]
async fn update_object_description_handler(_app: AppHandle, object_id: String, description: String) -> Result<(), String> {
    let mut conn = db::open_database()?;
    services::ingest::update_object_description(&mut conn, &object_id, &description)
}

#[tauri::command]
async fn verify_object_integrity(_app: AppHandle, object_id: String) -> Result<bool, String> {
    let conn = db::open_database()?;
    services::ingest::verify_object_integrity(&conn, &object_id)
}

#[tauri::command]
async fn verify_archive_fixity_command(
    _app: AppHandle,
) -> Result<services::fixity::FixityReport, String> {
    let conn = db::open_database()?;
    services::fixity::verify_archive_fixity(&conn)
}

#[tauri::command]
async fn generate_thumbnails_command(_app: AppHandle) -> Result<usize, String> {
    let conn = db::open_database()?;
    services::thumbnails::generate_missing_thumbnails(&conn)
}

#[tauri::command]
async fn get_thumbnail_command(_app: AppHandle, objectId: String) -> Result<Option<String>, String> {
    services::thumbnails::get_thumbnail_base64(&objectId)
}

#[tauri::command]
async fn get_document_card_command(_app: AppHandle, object_id: String) -> Result<services::card::DocumentCard, String> {
    let conn = db::open_database()?;
    services::card::get_document_card(&conn, &object_id)
}

#[tauri::command]
async fn update_document_card_command(_app: AppHandle, card: services::card::CardUpdate) -> Result<(), String> {
    let conn = db::open_database()?;
    services::card::update_document_card(&conn, &card)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            greet, 
            ingest_file, 
            ingest_directory_command,
            export_objects_csv_command,
            export_objects_json_command,
            index_reference_file,
            create_backup_command,
            restore_backup_command,
            reconcile_reference_locations,
            get_archive_objects, 
            get_object_tags, 
            add_tag_to_object,
            save_export_command,
            list_exports_command,
            rename_export_command,
            verify_archive_fixity_command,
            export_archive_csv,
            export_archive_json,
            get_object_persons,
            add_person_to_object,
            get_object_locations,
            add_location_to_object,
            get_all_tags_modal,
            update_tag_name,
            delete_tag_record,
            merge_tags_handler,
            get_all_persons_modal,
            update_person_name,
            delete_person_record,
            merge_persons_handler,
            get_all_locations_modal,
            update_location_name,
            delete_location_record,
            merge_locations_handler,
            get_audit_logs_modal,
            search_objects_filtered,
            search_objects_v2,
            update_object_description_handler,
            verify_object_integrity,
            count_objects_command,
            generate_thumbnails_command,
            get_thumbnail_command,
            get_document_card_command,
            update_document_card_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
