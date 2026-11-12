-- 20261110000003_add_exams_and_assignments.sql
-- Academic Examinations and Assignments Recording Engine Schema

-- 1. EXAMS TABLE
CREATE TABLE IF NOT EXISTS exams (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    term_id UUID REFERENCES academic_terms(id) ON DELETE SET NULL,
    class_section_id UUID NOT NULL REFERENCES class_sections(id) ON DELETE CASCADE,
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    exam_type VARCHAR(50) NOT NULL DEFAULT 'SUMMATIVE', -- 'SUMMATIVE', 'FORMATIVE', 'PRACTICE_TEST'
    affects_overall BOOLEAN NOT NULL DEFAULT true,      -- false if practice test/diagnostic
    exam_date DATE NOT NULL,
    start_time TIME,
    end_time TIME,
    max_marks NUMERIC(6, 2) NOT NULL DEFAULT 100.00,
    pass_marks NUMERIC(6, 2) DEFAULT 35.00,
    question_config JSONB NOT NULL DEFAULT '[]'::jsonb, -- e.g. [{"id": "q1", "label": "Q1", "max_marks": 10}]
    status VARCHAR(50) NOT NULL DEFAULT 'SCHEDULED',    -- 'SCHEDULED', 'COMPLETED', 'CANCELLED'
    whatsapp_notified BOOLEAN NOT NULL DEFAULT false,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 2. ASSIGNMENTS TABLE
CREATE TABLE IF NOT EXISTS assignments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    class_section_id UUID NOT NULL REFERENCES class_sections(id) ON DELETE CASCADE,
    subject_id UUID NOT NULL REFERENCES subjects(id) ON DELETE CASCADE,
    teacher_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title VARCHAR(255) NOT NULL,
    description TEXT,
    due_date DATE NOT NULL,
    max_marks NUMERIC(6, 2) NOT NULL DEFAULT 100.00,
    affects_overall BOOLEAN NOT NULL DEFAULT false,
    assignment_type VARCHAR(50) NOT NULL DEFAULT 'HOMEWORK', -- 'HOMEWORK', 'PROJECT', 'CLASSWORK', 'PRACTICE_TEST'
    question_config JSONB NOT NULL DEFAULT '[]'::jsonb,
    whatsapp_notified BOOLEAN NOT NULL DEFAULT false,
    status VARCHAR(50) NOT NULL DEFAULT 'ACTIVE',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 3. EXAM MARKS TABLE (Recording Engine)
CREATE TABLE IF NOT EXISTS exam_marks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    exam_id UUID NOT NULL REFERENCES exams(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    marks_obtained NUMERIC(6, 2),
    is_absent BOOLEAN NOT NULL DEFAULT false,
    question_marks JSONB NOT NULL DEFAULT '[]'::jsonb, -- e.g. [{"question_id": "q1", "marks": 9.5}]
    remarks TEXT,
    entered_by UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_exam_student UNIQUE(exam_id, student_id)
);

-- 4. ASSIGNMENT MARKS TABLE (Recording Engine)
CREATE TABLE IF NOT EXISTS assignment_marks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    assignment_id UUID NOT NULL REFERENCES assignments(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    marks_obtained NUMERIC(6, 2),
    is_absent BOOLEAN NOT NULL DEFAULT false,
    question_marks JSONB NOT NULL DEFAULT '[]'::jsonb,
    remarks TEXT,
    entered_by UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_assignment_student UNIQUE(assignment_id, student_id)
);

-- 5. PERFORMANCE INDEXES
CREATE INDEX IF NOT EXISTS idx_exams_school_section ON exams(school_id, class_section_id);
CREATE INDEX IF NOT EXISTS idx_exams_school_subject ON exams(school_id, subject_id);
CREATE INDEX IF NOT EXISTS idx_exams_school_date ON exams(school_id, exam_date);

CREATE INDEX IF NOT EXISTS idx_assignments_school_section ON assignments(school_id, class_section_id);
CREATE INDEX IF NOT EXISTS idx_assignments_school_subject ON assignments(school_id, subject_id);
CREATE INDEX IF NOT EXISTS idx_assignments_teacher ON assignments(school_id, teacher_user_id);

CREATE INDEX IF NOT EXISTS idx_exam_marks_exam_id ON exam_marks(exam_id);
CREATE INDEX IF NOT EXISTS idx_exam_marks_student_id ON exam_marks(student_id);

CREATE INDEX IF NOT EXISTS idx_assignment_marks_assignment_id ON assignment_marks(assignment_id);
CREATE INDEX IF NOT EXISTS idx_assignment_marks_student_id ON assignment_marks(student_id);

-- 6. TRIGGERS
DROP TRIGGER IF EXISTS set_timestamp_exams ON exams;
CREATE TRIGGER set_timestamp_exams BEFORE UPDATE ON exams FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

DROP TRIGGER IF EXISTS set_timestamp_assignments ON assignments;
CREATE TRIGGER set_timestamp_assignments BEFORE UPDATE ON assignments FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

DROP TRIGGER IF EXISTS set_timestamp_exam_marks ON exam_marks;
CREATE TRIGGER set_timestamp_exam_marks BEFORE UPDATE ON exam_marks FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

DROP TRIGGER IF EXISTS set_timestamp_assignment_marks ON assignment_marks;
CREATE TRIGGER set_timestamp_assignment_marks BEFORE UPDATE ON assignment_marks FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
