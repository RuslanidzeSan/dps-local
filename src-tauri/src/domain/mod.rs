//! Доменні сутності локального архіву (ADR-001, рішення 3).
//! Поклади цей файл у `src-tauri/src/domain/mod.rs`.
//!
//! Це чисті структури даних без логіки БД — схема віддзеркалює
//! міграцію 001_initial_schema. Логіка (інгест, аудит) житиме у шарах
//! services/db і працюватиме з цими типами.

use chrono::{DateTime, Utc};

/// Ролі користувачів (розділ 1.13 вихідного ТЗ).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserRole {
    User,
    Archivist,
    Administrator,
}

impl UserRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserRole::User => "User",
            UserRole::Archivist => "Archivist",
            UserRole::Administrator => "Administrator",
        }
    }
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: String, // UUID
    pub username: String,
    pub full_name: String,
    pub role: UserRole,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
}

/// Статус життєвого циклу файлу (розділ 1.3.1 ТЗ).
/// У MVP нові об'єкти завжди `Active` (ADR-001, рішення 1);
/// решта значень закладена для майбутніх етапів.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchiveObjectStatus {
    Active,
    Expired,
    PendingDeletion,
    PendingTransfer,
    Archived,
    Rejected,
    Destroyed,
}

impl ArchiveObjectStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ArchiveObjectStatus::Active => "Active",
            ArchiveObjectStatus::Expired => "Expired",
            ArchiveObjectStatus::PendingDeletion => "Pending_Deletion",
            ArchiveObjectStatus::PendingTransfer => "Pending_Transfer",
            ArchiveObjectStatus::Archived => "Archived",
            ArchiveObjectStatus::Rejected => "Rejected",
            ArchiveObjectStatus::Destroyed => "Destroyed",
        }
    }
}

/// Логічна картка архівного об'єкта. `id` — стабільний UUID,
/// `record_number` — внутрішній порядковий номер (ADR-001, рішення 2).
#[derive(Debug, Clone)]
pub struct ArchiveObject {
    pub id: String, // UUID
    pub record_number: i64,
    pub display_name: String,
    pub original_name: Option<String>,
    pub status: ArchiveObjectStatus,
    /// Ланцюжок версій: у першої версії дорівнює власному id (ТЗ, Document_Family_ID).
    pub document_family_id: String, // UUID
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Тип цифрового представлення. Оригінал не змінюється непомітно (Brief, принцип 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepresentationType {
    Original,
    Derivative,
}

impl RepresentationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            RepresentationType::Original => "original",
            RepresentationType::Derivative => "derivative",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Representation {
    pub id: String, // UUID
    pub object_id: String, // UUID → ArchiveObject
    pub representation_type: RepresentationType,
    pub created_at: DateTime<Utc>,
}

/// Де фізично лежать байти представлення.
/// Хеш і розмір — властивість саме копії, а не об'єкта (ADR-001, рішення 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageType {
    /// Копія у керованому сховищі програми.
    Managed,
    /// Зовнішній шлях без копіювання (Етап 5).
    Reference,
}

impl StorageType {
    pub fn as_str(&self) -> &'static str {
        match self {
            StorageType::Managed => "managed",
            StorageType::Reference => "reference",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ArchiveObjectSummary {
    pub id: String,
    pub display_name: String,
    pub status: String,
    pub file_path: String,
    pub file_size_bytes: i64,
    pub checksum_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct StorageLocation {
    pub id: String, // UUID
    pub representation_id: String, // UUID → Representation
    pub storage_type: StorageType,
    pub file_path: String,
    pub file_size_bytes: i64,
    pub checksum_sha256: String,
    pub is_accessible: bool,
    pub created_at: DateTime<Utc>,
}

/// Стан завдання імпорту (Етап 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestJobStatus {
    Pending,
    Processing,
    Completed,
    Failed,
}

impl IngestJobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            IngestJobStatus::Pending => "pending",
            IngestJobStatus::Processing => "processing",
            IngestJobStatus::Completed => "completed",
            IngestJobStatus::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct IngestJob {
    pub id: String, // UUID
    pub status: IngestJobStatus,
    pub source_path: String,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
}

/// Подія аудиту. Лише читання: система не надає API для зміни записів.
#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub id: String, // UUID
    pub user_id: String, // UUID → User
    pub action: String,
    pub target_table: String,
    pub target_id: String,
    pub payload: Option<String>,
    pub created_at: DateTime<Utc>,
}
