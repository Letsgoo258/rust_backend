-- 20261101000001_add_elective_groups_and_student_subject_enrollments.sql
-- Elective subject groups, student segregation per subject, parallel timetable slots, and attendance session updates.

-- 1. ELECTIVE GROUPS
CREATE TABLE IF NOT EXISTS elective_groups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    class_id UUID REFERENCES classes(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    code VARCHAR(50),
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_school_elective_group UNIQUE (school_id, academic_year_id, class_id, name)
);

CREATE INDEX IF NOT EXISTS idx_elective_groups_school_year ON elective_groups (school_id, academic_year_id);
CREATE INDEX IF NOT EXISTS idx_elective_groups_class ON elective_groups (school_id, class_id);

-- 2. ENHANCE SUBJECTS WITH ELECTIVE GROUP
ALTER TABLE subjects ADD COLUMN IF NOT EXISTS elective_group_id UUID REFERENCES elective_groups(id) ON DELETE SET NULL;

-- 3. STUDENT SUBJECT ENROLLMENTS (Subject-Based Segregation)
CREATE TABLE IF NOT EXISTS student_subject_enrollments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
    class_section_id UUID NOT NULL REFERENCES class_sections(id) ON DELETE CASCADE,
    elective_group_id UUID REFERENCES elective_groups(id) ON DELETE SET NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'DROPPED', 'COMPLETED')),
    enrolled_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_student_subject_year UNIQUE (student_id, subject_id, academic_year_id)
);

CREATE INDEX IF NOT EXISTS idx_student_subject_sec ON student_subject_enrollments (school_id, class_section_id, subject_id, status);
CREATE INDEX IF NOT EXISTS idx_student_subject_student ON student_subject_enrollments (school_id, student_id, academic_year_id);
CREATE INDEX IF NOT EXISTS idx_student_subject_group ON student_subject_enrollments (school_id, elective_group_id, status);

-- 4. UPDATE TIMETABLE_SLOTS TO ALLOW PARALLEL ELECTIVES IN SAME PERIOD
ALTER TABLE timetable_slots ADD COLUMN IF NOT EXISTS elective_group_id UUID REFERENCES elective_groups(id) ON DELETE SET NULL;
ALTER TABLE timetable_slots ADD COLUMN IF NOT EXISTS is_elective_slot BOOLEAN NOT NULL DEFAULT FALSE;

-- Drop previous unique constraint that blocked multiple subjects in the same period
ALTER TABLE timetable_slots DROP CONSTRAINT IF EXISTS uq_section_day_period;

-- Add updated unique constraint allowing parallel slots as long as subjects differ
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'uq_section_day_period_subject'
    ) THEN
        ALTER TABLE timetable_slots ADD CONSTRAINT uq_section_day_period_subject 
            UNIQUE (class_section_id, day_of_week, period_number, subject_id);
    END IF;
END $$;

CREATE INDEX IF NOT EXISTS idx_timetable_section_day_period ON timetable_slots (school_id, class_section_id, day_of_week, period_number);

-- 5. UPDATE ATTENDANCE_SESSIONS UNIQUE INDEX
-- Include subject_id so independent attendance sessions can be recorded for parallel elective subjects in the same period
DROP INDEX IF EXISTS uq_attendance_session_idx;

CREATE UNIQUE INDEX IF NOT EXISTS uq_attendance_session_idx 
    ON attendance_sessions (
        school_id, 
        academic_year_id, 
        class_section_id, 
        date, 
        COALESCE(period_number, 0), 
        COALESCE(subject_id, '00000000-0000-0000-0000-000000000000'::uuid)
    );

-- 6. Updated at triggers
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'set_timestamp_elective_groups') THEN
        CREATE TRIGGER set_timestamp_elective_groups BEFORE UPDATE ON elective_groups FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'set_timestamp_student_subject_enrollments') THEN
        CREATE TRIGGER set_timestamp_student_subject_enrollments BEFORE UPDATE ON student_subject_enrollments FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
    END IF;
END $$;
