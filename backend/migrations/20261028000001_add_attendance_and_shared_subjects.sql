-- 20261028000001_add_attendance_and_shared_subjects.sql
-- Complete attendance subsystem with multi-teacher shared subjects,
-- collision tracking, and timetable synchronizations.

-- 1. ENHANCE SUBJECT_TYPES FOR SHARED / MULTI-TEACHER SUBJECTS
ALTER TABLE subject_types ADD COLUMN IF NOT EXISTS allow_multiple_teachers BOOLEAN NOT NULL DEFAULT FALSE;

-- Update known collaborative types
UPDATE subject_types 
SET allow_multiple_teachers = TRUE 
WHERE LOWER(system_subject_type) IN ('practical', 'lab', 'vocational', 'elective', 'non-core')
   OR LOWER(name) LIKE '%lab%'
   OR LOWER(name) LIKE '%practical%'
   OR LOWER(name) LIKE '%physical education%'
   OR LOWER(name) LIKE '%sports%';

-- 2. ENHANCE TIMETABLE_SLOTS & CURRENT_DAY_TIMETABLE FOR MULTI-TEACHER
ALTER TABLE timetable_slots ADD COLUMN IF NOT EXISTS co_teacher_staff_ids UUID[] NOT NULL DEFAULT '{}';
ALTER TABLE timetable_slots ADD COLUMN IF NOT EXISTS is_shared BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE current_day_timetable ADD COLUMN IF NOT EXISTS co_teacher_ids UUID[] NOT NULL DEFAULT '{}';
ALTER TABLE current_day_timetable ADD COLUMN IF NOT EXISTS is_shared_subject BOOLEAN NOT NULL DEFAULT FALSE;

-- 3. ATTENDANCE SESSIONS
CREATE TABLE IF NOT EXISTS attendance_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    class_section_id UUID NOT NULL REFERENCES class_sections(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    period_number INT CHECK (period_number BETWEEN 1 AND 12),
    attendance_type VARCHAR(20) NOT NULL DEFAULT 'DAILY' CHECK (attendance_type IN ('DAILY', 'PERIOD_WISE')),
    subject_id UUID REFERENCES subjects(id) ON DELETE SET NULL,
    marked_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    marked_by_role VARCHAR(50) NOT NULL,
    co_marked_by UUID[] NOT NULL DEFAULT '{}',
    is_shared_session BOOLEAN NOT NULL DEFAULT FALSE,
    status VARCHAR(20) NOT NULL DEFAULT 'RECORDED' CHECK (status IN ('RECORDED', 'LOCKED', 'FLAGGED')),
    is_locked BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_attendance_session_idx 
    ON attendance_sessions (school_id, academic_year_id, class_section_id, date, COALESCE(period_number, 0));

CREATE INDEX IF NOT EXISTS idx_attendance_sessions_school_date 
    ON attendance_sessions (school_id, date, class_section_id);

CREATE INDEX IF NOT EXISTS idx_attendance_sessions_marked_by 
    ON attendance_sessions (school_id, marked_by, date);

-- 4. ATTENDANCE RECORDS (STUDENT MARKS)
CREATE TABLE IF NOT EXISTS attendance_records (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    attendance_session_id UUID NOT NULL REFERENCES attendance_sessions(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    status VARCHAR(20) NOT NULL CHECK (status IN ('PRESENT', 'ABSENT', 'LATE', 'EXCUSED', 'HALF_DAY')),
    marked_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    marked_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    remarks TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_attendance_record UNIQUE (attendance_session_id, student_id)
);

CREATE INDEX IF NOT EXISTS idx_attendance_records_student_date 
    ON attendance_records (school_id, student_id, marked_at);

CREATE INDEX IF NOT EXISTS idx_attendance_records_session_status 
    ON attendance_records (attendance_session_id, status);

-- 5. ATTENDANCE CORRECTIONS (AUDIT TRAIL)
CREATE TABLE IF NOT EXISTS attendance_corrections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    attendance_record_id UUID NOT NULL REFERENCES attendance_records(id) ON DELETE CASCADE,
    attendance_session_id UUID NOT NULL REFERENCES attendance_sessions(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    old_status VARCHAR(20) NOT NULL,
    new_status VARCHAR(20) NOT NULL,
    reason TEXT NOT NULL,
    changed_by UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
    changed_by_role VARCHAR(50) NOT NULL,
    is_critical BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_attendance_corrections_session 
    ON attendance_corrections (school_id, attendance_session_id);

CREATE INDEX IF NOT EXISTS idx_attendance_corrections_changed_by 
    ON attendance_corrections (school_id, changed_by, created_at);

-- 6. ATTENDANCE COLLISIONS & TICKET MAPPING
CREATE TABLE IF NOT EXISTS attendance_collisions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    class_section_id UUID NOT NULL REFERENCES class_sections(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    period_number INT,
    attempted_by_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    attempted_by_role VARCHAR(50) NOT NULL,
    existing_session_id UUID REFERENCES attendance_sessions(id) ON DELETE SET NULL,
    existing_marked_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    collision_type VARCHAR(50) NOT NULL,
    ticket_id UUID REFERENCES data_change_tickets(id) ON DELETE SET NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'REPORTED' CHECK (status IN ('REPORTED', 'RESOLVED', 'DISMISSED')),
    resolution_notes TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_attendance_collisions_lookup 
    ON attendance_collisions (school_id, date, class_section_id, status);

-- 7. ATTACH TIMESTAMPTZ TRIGGERS
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'set_timestamp_attendance_sessions') THEN
        CREATE TRIGGER set_timestamp_attendance_sessions 
        BEFORE UPDATE ON attendance_sessions 
        FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
    END IF;

    IF NOT EXISTS (SELECT 1 FROM pg_trigger WHERE tgname = 'set_timestamp_attendance_records') THEN
        CREATE TRIGGER set_timestamp_attendance_records 
        BEFORE UPDATE ON attendance_records 
        FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
    END IF;
END $$;

-- 8. UPDATE DAILY TIMETABLE STORED PROCEDURE TO INCLUDE CO-TEACHERS & SHARED FLAG
CREATE OR REPLACE FUNCTION sync_daily_timetable_for_date(p_date DATE)
RETURNS INTEGER AS $$
DECLARE
    v_dow INT;
    v_inserted INT := 0;
BEGIN
    v_dow := EXTRACT(ISODOW FROM p_date); -- 1 = Monday, 7 = Sunday

    INSERT INTO current_day_timetable (
        school_id, date, timetable_slot_id, class_section_id,
        day_of_week, period_number, start_time, end_time,
        subject_id, effective_teacher_id, original_teacher_id,
        co_teacher_ids, is_shared_subject,
        substitution_id, is_substituted, room_id, status, created_at, updated_at
    )
    SELECT
        ts.school_id,
        p_date,
        ts.id,
        ts.class_section_id,
        ts.day_of_week,
        ts.period_number,
        ts.start_time,
        ts.end_time,
        ts.subject_id,
        COALESCE(sub.substitute_teacher_id, ts.teacher_staff_id) as effective_teacher_id,
        ts.teacher_staff_id as original_teacher_id,
        ts.co_teacher_staff_ids as co_teacher_ids,
        ts.is_shared as is_shared_subject,
        sub.id as substitution_id,
        (sub.id IS NOT NULL) as is_substituted,
        ts.room_id,
        'SCHEDULED',
        NOW(),
        NOW()
    FROM timetable_slots ts
    LEFT JOIN substitutions sub ON sub.timetable_slot_id = ts.id AND sub.date = p_date AND sub.status = 'ACTIVE'
    WHERE ts.day_of_week = v_dow AND ts.status = 'ACTIVE'
    ON CONFLICT (timetable_slot_id, date) DO UPDATE SET
        class_section_id = EXCLUDED.class_section_id,
        day_of_week = EXCLUDED.day_of_week,
        period_number = EXCLUDED.period_number,
        start_time = EXCLUDED.start_time,
        end_time = EXCLUDED.end_time,
        subject_id = EXCLUDED.subject_id,
        effective_teacher_id = EXCLUDED.effective_teacher_id,
        original_teacher_id = EXCLUDED.original_teacher_id,
        co_teacher_ids = EXCLUDED.co_teacher_ids,
        is_shared_subject = EXCLUDED.is_shared_subject,
        substitution_id = EXCLUDED.substitution_id,
        is_substituted = EXCLUDED.is_substituted,
        room_id = EXCLUDED.room_id,
        updated_at = NOW();

    GET DIAGNOSTICS v_inserted = ROW_COUNT;

    -- Prune historic rows older than 30 days
    DELETE FROM current_day_timetable WHERE date < (p_date - INTERVAL '30 days');

    RETURN v_inserted;
END;
$$ LANGUAGE plpgsql;
