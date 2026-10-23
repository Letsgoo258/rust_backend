-- 20261023000004_add_class_id_to_subjects.sql
ALTER TABLE subjects ADD COLUMN IF NOT EXISTS class_id UUID REFERENCES classes(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_subjects_school_class ON subjects (school_id, class_id);
