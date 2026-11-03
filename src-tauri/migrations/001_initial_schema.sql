CREATE TABLE IF NOT EXISTS users (
  id TEXT PRIMARY KEY, username TEXT NOT NULL UNIQUE, full_name TEXT NOT NULL,
  role TEXT NOT NULL CHECK(role IN ('User','Archivist','Administrator')),
  is_active INTEGER NOT NULL DEFAULT 1 CHECK(is_active IN (0,1)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
INSERT OR IGNORE INTO users(id,username,full_name,role) VALUES
('00000000-0000-4000-8000-000000000001','local-archivist','Local Archivist','Archivist');

CREATE TABLE IF NOT EXISTS archive_objects (
  id TEXT PRIMARY KEY, record_number INTEGER UNIQUE, display_name TEXT NOT NULL,
  original_name TEXT NOT NULL, file_extension TEXT, description TEXT,
  status TEXT NOT NULL DEFAULT 'Active' CHECK(status IN ('Active','Expired','Pending_Deletion','Pending_Transfer','Archived','Rejected','Destroyed')),
  document_family_id TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS representations (
  id TEXT PRIMARY KEY, object_id TEXT NOT NULL REFERENCES archive_objects(id) ON DELETE RESTRICT,
  representation_type TEXT NOT NULL CHECK(representation_type IN ('original','derivative')), created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS storage_locations (
  id TEXT PRIMARY KEY, representation_id TEXT NOT NULL REFERENCES representations(id) ON DELETE RESTRICT,
  storage_type TEXT NOT NULL CHECK(storage_type IN ('managed','reference')), file_path TEXT NOT NULL,
  file_size_bytes INTEGER NOT NULL CHECK(file_size_bytes >= 0), checksum_sha256 TEXT,
  is_accessible INTEGER NOT NULL DEFAULT 1 CHECK(is_accessible IN (0,1)), created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS ingest_jobs (
  id TEXT PRIMARY KEY, status TEXT NOT NULL CHECK(status IN ('pending','processing','completed','failed')),
  source_path TEXT NOT NULL, source_sha256 TEXT, object_id TEXT REFERENCES archive_objects(id),
  created_at TEXT NOT NULL, completed_at TEXT, error_message TEXT
);
CREATE TABLE IF NOT EXISTS audit_events (
  id TEXT PRIMARY KEY, user_id TEXT NOT NULL REFERENCES users(id), action TEXT NOT NULL,
  target_table TEXT NOT NULL, target_id TEXT NOT NULL, payload TEXT, created_at TEXT NOT NULL
);
CREATE TRIGGER IF NOT EXISTS trg_audit_events_no_update BEFORE UPDATE ON audit_events BEGIN SELECT RAISE(ABORT, 'audit_events is append-only'); END;
CREATE TRIGGER IF NOT EXISTS trg_audit_events_no_delete BEFORE DELETE ON audit_events BEGIN SELECT RAISE(ABORT, 'audit_events is append-only'); END;
-- Compatibility projection for the current catalogue UI; it is append-only too.
CREATE TABLE IF NOT EXISTS audit_logs (id TEXT PRIMARY KEY, action_type TEXT NOT NULL, target_record_id TEXT, details TEXT, created_at TEXT NOT NULL);
CREATE TRIGGER IF NOT EXISTS trg_audit_logs_no_update BEFORE UPDATE ON audit_logs BEGIN SELECT RAISE(ABORT, 'audit_logs is append-only'); END;
CREATE TRIGGER IF NOT EXISTS trg_audit_logs_no_delete BEFORE DELETE ON audit_logs BEGIN SELECT RAISE(ABORT, 'audit_logs is append-only'); END;
CREATE TABLE IF NOT EXISTS ref_tags (id TEXT PRIMARY KEY, tag_name TEXT NOT NULL UNIQUE COLLATE NOCASE, is_active INTEGER NOT NULL DEFAULT 1 CHECK(is_active IN (0,1)), created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS ref_persons (id TEXT PRIMARY KEY, person_name TEXT NOT NULL UNIQUE COLLATE NOCASE, is_active INTEGER NOT NULL DEFAULT 1 CHECK(is_active IN (0,1)), created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS ref_locations (id TEXT PRIMARY KEY, location_name TEXT NOT NULL UNIQUE COLLATE NOCASE, is_active INTEGER NOT NULL DEFAULT 1 CHECK(is_active IN (0,1)), created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS map_file_tags (file_id TEXT NOT NULL REFERENCES archive_objects(id) ON DELETE RESTRICT, tag_id TEXT NOT NULL REFERENCES ref_tags(id) ON DELETE RESTRICT, PRIMARY KEY(file_id,tag_id));
CREATE TABLE IF NOT EXISTS map_file_persons (file_id TEXT NOT NULL REFERENCES archive_objects(id) ON DELETE RESTRICT, person_id TEXT NOT NULL REFERENCES ref_persons(id) ON DELETE RESTRICT, PRIMARY KEY(file_id,person_id));
CREATE TABLE IF NOT EXISTS map_file_locations (file_id TEXT NOT NULL REFERENCES archive_objects(id) ON DELETE RESTRICT, location_id TEXT NOT NULL REFERENCES ref_locations(id) ON DELETE RESTRICT, PRIMARY KEY(file_id,location_id));
CREATE INDEX IF NOT EXISTS idx_archive_objects_name ON archive_objects(display_name);
CREATE INDEX IF NOT EXISTS idx_storage_checksum ON storage_locations(checksum_sha256);
