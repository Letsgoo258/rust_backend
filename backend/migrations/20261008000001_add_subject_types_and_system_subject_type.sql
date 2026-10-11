-- 20261008000001_add_subject_types_and_system_subject_type.sql

CREATE TABLE IF NOT EXISTS subject_types (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    code VARCHAR(50) NOT NULL,
    system_subject_type VARCHAR(50) NOT NULL DEFAULT 'Core', -- 'Core', 'Practical', 'Vocational', 'Non-Core'
    is_elective BOOLEAN NOT NULL DEFAULT FALSE,
    is_mandatory BOOLEAN NOT NULL DEFAULT TRUE,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(school_id, code)
);

ALTER TABLE subjects ADD COLUMN IF NOT EXISTS system_subject_type VARCHAR(50) NOT NULL DEFAULT 'Core';
ALTER TABLE subjects ADD COLUMN IF NOT EXISTS subject_type_id UUID REFERENCES subject_types(id) ON DELETE SET NULL;
