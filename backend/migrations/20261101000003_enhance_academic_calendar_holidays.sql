-- Migration: 20261101000003_enhance_academic_calendar_holidays.sql
-- Supports school-wide and class-wise holiday / academic calendar registrations

ALTER TABLE academic_calendar
    ADD COLUMN IF NOT EXISTS applies_to_all_classes BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN IF NOT EXISTS class_ids UUID[] NOT NULL DEFAULT '{}',
    ADD COLUMN IF NOT EXISTS created_by UUID REFERENCES users(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_academic_calendar_school_date 
    ON academic_calendar(school_id, date);

CREATE INDEX IF NOT EXISTS idx_academic_calendar_school_year 
    ON academic_calendar(school_id, academic_year_id, date);

CREATE INDEX IF NOT EXISTS idx_academic_calendar_class_ids 
    ON academic_calendar USING GIN(class_ids);
