-- 20261111000001_exam_events_restructure.sql
-- Exam Events Parent Table and Multi-Class Scheduling Architecture

-- 1. Create exam_events table
CREATE TABLE IF NOT EXISTS exam_events (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,                         -- e.g. "Term 1 Examination", "Mid-Term Assessment"
    exam_type VARCHAR(50) NOT NULL DEFAULT 'SUMMATIVE', -- 'SUMMATIVE', 'FORMATIVE', 'PRACTICE_TEST'
    affects_overall BOOLEAN NOT NULL DEFAULT true,       -- whether affects overall marks/grade
    status VARCHAR(50) NOT NULL DEFAULT 'SCHEDULED',     -- 'SCHEDULED', 'ONGOING', 'COMPLETED', 'CANCELLED'
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 2. Add exam_event_id to exams table
ALTER TABLE exams
    ADD COLUMN IF NOT EXISTS exam_event_id UUID REFERENCES exam_events(id) ON DELETE CASCADE;

-- 3. Backfill existing exams into exam_events by distinct (school_id, academic_year_id, name)
INSERT INTO exam_events (school_id, academic_year_id, name, exam_type, affects_overall, created_by)
SELECT DISTINCT ON (school_id, academic_year_id, name)
    school_id, academic_year_id, name, exam_type, affects_overall, created_by
FROM exams
ON CONFLICT DO NOTHING;

-- 4. Link existing exams to their parent exam_events
UPDATE exams e
SET exam_event_id = ee.id
FROM exam_events ee
WHERE ee.school_id = e.school_id
  AND ee.academic_year_id = e.academic_year_id
  AND ee.name = e.name
  AND e.exam_event_id IS NULL;

-- 5. Indexes
CREATE INDEX IF NOT EXISTS idx_exam_events_school_year ON exam_events(school_id, academic_year_id);
CREATE INDEX IF NOT EXISTS idx_exam_events_status ON exam_events(school_id, status);
CREATE INDEX IF NOT EXISTS idx_exams_event_id ON exams(exam_event_id);

-- 6. Trigger for updated_at
DROP TRIGGER IF EXISTS set_timestamp_exam_events ON exam_events;
CREATE TRIGGER set_timestamp_exam_events BEFORE UPDATE ON exam_events FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
