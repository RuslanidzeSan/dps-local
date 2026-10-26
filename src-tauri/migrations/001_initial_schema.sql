-- 001_initial_schema.sql
-- Початкова схема локального архіву (Етап 2, ADR-001).
-- Часові позначки — ISO 8601, UTC (AC-20 вихідного ТЗ).
-- UUID зберігаються як TEXT у канонічному вигляді.

-- ---------------------------------------------------------------------------
-- Користувачі. У MVP один неявний локальний користувач (ADR-001, рішення 5),
-- але таблиця закладена з першої міграції для прив'язки User_ID в аудиті.
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS users (
    id          TEXT PRIMARY KEY,
    username    TEXT NOT NULL UNIQUE,
    full_name   TEXT NOT NULL,
    role        TEXT NOT NULL
                CHECK (role IN ('User', 'Archivist', 'Administrator')),
    is_active   INTEGER NOT NULL DEFAULT 1
                CHECK (is_active IN (0, 1)),
    created_at  TEXT NOT NULL
                DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Системний дефолтний користувач. INSERT OR IGNORE + фіксований UUID
-- гарантують, що повторне застосування не створить дублікат.
INSERT OR IGNORE INTO users (id, username, full_name, role)
VALUES (
    '00000000-0000-4000-8000-000000000001',
    'local-archivist',
    'Local Archivist',
    'Archivist'
);

-- ---------------------------------------------------------------------------
-- Логічні архівні об'єкти (картки).
-- Примітка (ADR, погоджена розбіжність): фізичним PK є record_number
-- (INTEGER PRIMARY KEY AUTOINCREMENT — вимога SQLite), а стабільний UUID у
-- полі `id` є логічним ідентифікатором, який бачитимуть UI та майбутній API.
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS archive_objects (
    record_number       INTEGER PRIMARY KEY AUTOINCREMENT,
    id                  TEXT NOT NULL UNIQUE,
    display_name        TEXT NOT NULL,
    original_name       TEXT,
    status              TEXT NOT NULL DEFAULT 'Active'
                        CHECK (status IN (
                            'Active', 'Expired', 'Pending_Deletion',
                            'Pending_Transfer', 'Archived', 'Rejected',
                            'Destroyed'
                        )),
    document_family_id  TEXT NOT NULL,
    created_at          TEXT NOT NULL
                        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at          TEXT NOT NULL
                        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- ---------------------------------------------------------------------------
-- Цифрові представлення об'єкта (оригінал, похідні версії).
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS representations (
    id                  TEXT PRIMARY KEY,
    object_id           TEXT NOT NULL REFERENCES archive_objects (id),
    representation_type TEXT NOT NULL DEFAULT 'original'
                        CHECK (representation_type IN ('original', 'derivative')),
    created_at          TEXT NOT NULL
                        DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- ---------------------------------------------------------------------------
-- Фізичні копії представлення. Хеш і розмір живуть САМЕ ТУТ (ADR-001,
-- рішення 3), а не в картці об'єкта. storage_type: 'managed' — копія у
-- сховищі програми; 'reference' — зовнішній шлях (Етап 5).
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS storage_locations (
    id                TEXT PRIMARY KEY,
    representation_id TEXT NOT NULL REFERENCES representations (id),
    storage_type      TEXT NOT NULL DEFAULT 'managed'
                      CHECK (storage_type IN ('managed', 'reference')),
    file_path         TEXT NOT NULL,
    file_size_bytes   INTEGER NOT NULL CHECK (file_size_bytes >= 0),
    checksum_sha256   TEXT NOT NULL,
    is_accessible     INTEGER NOT NULL DEFAULT 1
                      CHECK (is_accessible IN (0, 1)),
    created_at        TEXT NOT NULL
                      DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- ---------------------------------------------------------------------------
-- Завдання імпорту (Етап 3 буде їх використовувати).
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS ingest_jobs (
    id            TEXT PRIMARY KEY,
    status        TEXT NOT NULL DEFAULT 'pending'
                  CHECK (status IN ('pending', 'processing', 'completed', 'failed')),
    source_path   TEXT NOT NULL,
    created_at    TEXT NOT NULL
                  DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    completed_at  TEXT,
    error_message TEXT
);

-- ---------------------------------------------------------------------------
-- Аудит-лог: append-only (розділ 1.8 ТЗ, ADR-001, рішення 4).
-- Доменний шар пише сюди; guard-тригер нижче блокує підробку/стирання
-- навіть при прямому доступі до бази.
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS audit_events (
    id           TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES users (id),
    action       TEXT NOT NULL,
    target_table TEXT NOT NULL,
    target_id    TEXT NOT NULL,
    payload      TEXT,
    created_at   TEXT NOT NULL
                 DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TRIGGER IF NOT EXISTS trg_audit_events_no_update
BEFORE UPDATE ON audit_events
BEGIN
    SELECT RAISE(ABORT, 'audit_events is append-only: UPDATE is forbidden');
END;

CREATE TRIGGER IF NOT EXISTS trg_audit_events_no_delete
BEFORE DELETE ON audit_events
BEGIN
    SELECT RAISE(ABORT, 'audit_events is append-only: DELETE is forbidden');
END;
