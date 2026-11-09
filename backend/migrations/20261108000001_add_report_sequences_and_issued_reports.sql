-- Migration: 20261108000001_add_report_sequences_and_issued_reports.sql
-- Supports report sequence locking, atomic serial number increments, and an audit registry of all issued certificates/reports

CREATE TABLE IF NOT EXISTS report_sequences (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    report_type VARCHAR(50) NOT NULL, -- e.g. 'transfer_certificate', 'bonafide_certificate', 'conduct_certificate', 'study_certificate', 'report_card', 'id_card'
    display_name VARCHAR(100) NOT NULL DEFAULT 'Transfer Certificate',
    prefix VARCHAR(20) NOT NULL DEFAULT 'TC',
    suffix VARCHAR(20) DEFAULT '',
    current_number INT NOT NULL DEFAULT 0,
    padding_length INT NOT NULL DEFAULT 4,
    format_pattern VARCHAR(100) NOT NULL DEFAULT '{prefix}/{year}/{number}',
    academic_year VARCHAR(20),
    is_locked BOOLEAN NOT NULL DEFAULT false,
    reset_annually BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(school_id, report_type)
);

CREATE INDEX IF NOT EXISTS idx_report_sequences_school_type ON report_sequences(school_id, report_type);

CREATE TABLE IF NOT EXISTS issued_reports (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    report_type VARCHAR(50) NOT NULL,
    document_number VARCHAR(100) NOT NULL,
    serial_number INT NOT NULL,
    student_id UUID REFERENCES users(id) ON DELETE SET NULL,
    student_name VARCHAR(255) NOT NULL,
    admission_number VARCHAR(100),
    class_section VARCHAR(100),
    issued_by UUID REFERENCES users(id) ON DELETE SET NULL,
    issuer_name VARCHAR(255),
    issue_date DATE NOT NULL DEFAULT CURRENT_DATE,
    academic_year VARCHAR(20),
    status VARCHAR(30) NOT NULL DEFAULT 'ISSUED', -- 'ISSUED', 'CANCELLED', 'DUPLICATE'
    variables_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    template_version INT,
    remarks TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(school_id, report_type, document_number)
);

CREATE INDEX IF NOT EXISTS idx_issued_reports_school_type ON issued_reports(school_id, report_type, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_issued_reports_student ON issued_reports(school_id, student_id);
CREATE INDEX IF NOT EXISTS idx_issued_reports_doc_no ON issued_reports(school_id, document_number);
