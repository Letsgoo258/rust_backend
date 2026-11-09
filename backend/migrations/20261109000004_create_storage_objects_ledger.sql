-- 20261109000004_create_storage_objects_ledger.sql
-- Storage Ledger for ImageKit assets, encryption at rest, and Google Drive backup compliance

CREATE TABLE IF NOT EXISTS storage_objects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID REFERENCES schools(id) ON DELETE CASCADE,
    entity_type VARCHAR(50) NOT NULL, -- 'SCHOOL_LOGO', 'HEADER_BANNER', 'ADMISSION_DOCUMENT', 'STUDENT_PHOTO', 'TRANSFER_CERTIFICATE'
    entity_id VARCHAR(100),           -- Target entity reference (admission ID, student ID, template ID, etc.)
    file_id VARCHAR(100),             -- ImageKit file ID for remote lifecycle management
    file_name VARCHAR(255) NOT NULL,
    file_path VARCHAR(500) NOT NULL,
    url TEXT NOT NULL,
    storage_provider VARCHAR(50) NOT NULL DEFAULT 'IMAGEKIT', -- 'IMAGEKIT', 'GOOGLE_DRIVE', 'LOCAL'
    is_encrypted BOOLEAN NOT NULL DEFAULT FALSE,
    encryption_iv VARCHAR(64),        -- Hex-encoded IV/nonce if encrypted with AES-256-GCM
    checksum_sha256 VARCHAR(64) NOT NULL,
    file_size_bytes BIGINT NOT NULL DEFAULT 0,
    mime_type VARCHAR(100) NOT NULL DEFAULT 'application/octet-stream',
    google_backup_status VARCHAR(50) NOT NULL DEFAULT 'PENDING', -- 'PENDING', 'BACKED_UP', 'FAILED', 'NOT_REQUIRED'
    google_backup_file_id VARCHAR(255),
    google_backup_at TIMESTAMPTZ,
    retention_until TIMESTAMPTZ,      -- Expiration date after which document is purged from ImageKit
    deleted_from_provider_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_storage_objects_school_entity 
ON storage_objects (school_id, entity_type, entity_id);

CREATE INDEX IF NOT EXISTS idx_storage_objects_file_id 
ON storage_objects (file_id);

CREATE INDEX IF NOT EXISTS idx_storage_objects_google_backup 
ON storage_objects (google_backup_status) 
WHERE google_backup_status = 'PENDING';

CREATE INDEX IF NOT EXISTS idx_storage_objects_retention 
ON storage_objects (retention_until) 
WHERE retention_until IS NOT NULL AND deleted_from_provider_at IS NULL;
