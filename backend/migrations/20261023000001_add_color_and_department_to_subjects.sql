-- 20261023000001_add_color_and_department_to_subjects.sql

ALTER TABLE subjects ADD COLUMN IF NOT EXISTS color VARCHAR(20) DEFAULT '#4F46E5';
ALTER TABLE subjects ADD COLUMN IF NOT EXISTS department VARCHAR(100);
