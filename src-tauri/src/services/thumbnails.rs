use base64::Engine;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use crate::db;

/// Максимальний розмір мініатюри по довшій стороні.
const THUMB_MAX: u32 = 320;
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "webp", "bmp", "tif", "tiff"];

/// Чи вважаємо розширення зображенням (регістр і крапка не важливі).
pub fn is_image_extension(ext: Option<&str>) -> bool {
    match ext {
        Some(e) => IMAGE_EXTENSIONS.contains(
            &e.trim().trim_start_matches('.').to_ascii_lowercase().as_str(),
        ),
        None => false,
    }
}

/// Створює JPEG-мініатюру для файла-джерела.
/// Ok(false) = джерело не зображення або недоступне (не помилка, а пропуск).
pub fn generate_thumbnail_at(source: &Path, dest: &Path) -> Result<bool, String> {
    // Визначаємо формат за ВМІСТОМ файлу (магічні байти), а не за розширенням —
    // надійніше для копій у сховищі.
    let img = match image::ImageReader::open(source) {
        Ok(reader) => match reader.with_guessed_format() {
            Ok(guessing) => match guessing.decode() {
                Ok(i) => i,
                Err(_) => return Ok(false), // не зображення
            },
            Err(_) => return Ok(false), // не вдалося визначити формат
        },
        Err(_) => return Ok(false), // файл недоступний
    };
    let thumb = img.thumbnail(THUMB_MAX, THUMB_MAX);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    thumb
        .save_with_format(dest, image::ImageFormat::Jpeg)
        .map_err(|e| e.to_string())?;
    Ok(true)
}

/// Знаходить усі managed-об'єкти-зображення без мініатюр і генерує їх.
/// Повертає кількість створених мініатюр. Безпечно запускати повторно.
pub fn generate_missing_thumbnails_at(conn: &Connection, thumbs_dir: &Path) -> Result<usize, String> {
    let rows = {
        let mut stmt = conn.prepare(
            "SELECT o.id, o.file_extension, s.file_path \
             FROM archive_objects o \
             JOIN representations r ON r.object_id = o.id \
             JOIN storage_locations s ON s.representation_id = r.id \
             WHERE s.storage_type = 'managed'",
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
            ))
        }).map_err(|e| e.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())?
    };

    let mut generated = 0;
    for (object_id, ext, file_path) in rows {
        if !is_image_extension(ext.as_deref()) { continue; }
        let dest = thumbs_dir.join(format!("{object_id}.jpg"));
        if dest.exists() { continue; }               // вже є — пропускаємо
        let source = PathBuf::from(&file_path);
        if !source.is_file() { continue; }            // файл зник — пропускаємо
        if generate_thumbnail_at(&source, &dest)? {
            generated += 1;
        }
    }
    Ok(generated)
}

/// Мініатюра об'єкта у вигляді data-URL для <img src=...> у фронтенді.
/// None = мініатюри немає (не зображення або ще не згенеровано).
pub fn get_thumbnail_base64_at(thumbs_dir: &Path, object_id: &str) -> Result<Option<String>, String> {
    let dest = thumbs_dir.join(format!("{object_id}.jpg"));
    if !dest.is_file() { return Ok(None); }
    let bytes = std::fs::read(&dest).map_err(|e| e.to_string())?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(Some(format!("data:image/jpeg;base64,{b64}")))
}

// --- «Продуктові» обгортки з реальним шляхом ~/.dps-local/thumbnails/ ---

pub fn generate_missing_thumbnails(conn: &Connection) -> Result<usize, String> {
    generate_missing_thumbnails_at(conn, &db::get_thumbnails_dir()?)
}

pub fn get_thumbnail_base64(object_id: &str) -> Result<Option<String>, String> {
    get_thumbnail_base64_at(&db::get_thumbnails_dir()?, object_id)
}
