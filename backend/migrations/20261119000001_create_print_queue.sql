-- Migration: 20261119000001_create_print_queue.sql
-- Print Queue table for Print & Certificate Studio module
-- Retains metadata-based print jobs with 24-hour expiration window

CREATE TABLE IF NOT EXISTS print_queue (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    uploaded_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    document_type VARCHAR(50) NOT NULL,
    title VARCHAR(255) NOT NULL,
    recipient_name VARCHAR(255) NOT NULL,
    recipient_phone VARCHAR(50),
    recipient_admission_number VARCHAR(100),
    file_url TEXT,
    file_name VARCHAR(255),
    file_size_bytes BIGINT DEFAULT 0,
    copies INT DEFAULT 1,
    status VARCHAR(30) NOT NULL DEFAULT 'PENDING',
    options JSONB NOT NULL DEFAULT '{}'::jsonb,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL DEFAULT (now() + interval '24 hours'),
    printed_at TIMESTAMPTZ,
    printed_by UUID REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_print_queue_school_status ON print_queue(school_id, status, expires_at);
CREATE INDEX IF NOT EXISTS idx_print_queue_recipient ON print_queue(school_id, recipient_phone, recipient_admission_number);
CREATE INDEX IF NOT EXISTS idx_print_queue_uploaded_by ON print_queue(uploaded_by, expires_at);
CREATE INDEX IF NOT EXISTS idx_print_queue_expires ON print_queue(expires_at);
