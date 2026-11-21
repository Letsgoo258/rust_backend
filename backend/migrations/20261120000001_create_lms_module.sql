-- 20261120000001_create_lms_module.sql
-- ERAVAYA LMS Module Schema: Unified Tasks, Recipients, Attachments, Evaluations, Evaluation Marks, and Audit Logs

-- 1. LMS TASKS
CREATE TABLE IF NOT EXISTS lms_tasks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    title VARCHAR(255) NOT NULL,
    description TEXT NOT NULL,
    special_message TEXT,
    task_type VARCHAR(50) NOT NULL DEFAULT 'HOMEWORK', -- 'HOMEWORK', 'ASSIGNMENT', 'CLASSROOM_TASK', 'OTHER'
    class_section_id UUID NOT NULL REFERENCES class_sections(id) ON DELETE CASCADE,
    subject_id UUID REFERENCES subjects(id) ON DELETE SET NULL,
    teaching_group_id UUID,
    attendance_session_id UUID REFERENCES attendance_sessions(id) ON DELETE RESTRICT,
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deadline_at TIMESTAMPTZ,
    status VARCHAR(50) NOT NULL DEFAULT 'DRAFT', -- 'DRAFT', 'PUBLISHED', 'CLOSED', 'CANCELLED'
    add_to_calendar BOOLEAN NOT NULL DEFAULT FALSE,
    calendar_event_id UUID REFERENCES academic_calendar(id) ON DELETE SET NULL,
    notify_whatsapp BOOLEAN NOT NULL DEFAULT FALSE,
    notify_email BOOLEAN NOT NULL DEFAULT FALSE,
    is_evaluation BOOLEAN NOT NULL DEFAULT FALSE,
    published_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 2. LMS TASK RECIPIENTS (Audience snapshot at publication)
CREATE TABLE IF NOT EXISTS lms_task_recipients (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    task_id UUID NOT NULL REFERENCES lms_tasks(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    enrollment_id UUID REFERENCES enrollments(id) ON DELETE SET NULL,
    recipient_status VARCHAR(50) NOT NULL DEFAULT 'ASSIGNED', -- 'ASSIGNED', 'SUBMITTED', 'EVALUATED'
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_lms_task_recipient UNIQUE (task_id, student_id)
);

-- 3. LMS TASK ATTACHMENTS
CREATE TABLE IF NOT EXISTS lms_task_attachments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    task_id UUID NOT NULL REFERENCES lms_tasks(id) ON DELETE CASCADE,
    attachment_type VARCHAR(50) NOT NULL, -- 'TEMPORARY_UPLOAD', 'GOOGLE_DRIVE'
    original_filename VARCHAR(255),
    mime_type VARCHAR(100),
    file_size BIGINT,
    imagekit_file_id VARCHAR(255),
    file_url TEXT NOT NULL,
    google_drive_url TEXT,
    uploaded_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    uploaded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    deletion_status VARCHAR(50) NOT NULL DEFAULT 'ACTIVE', -- 'ACTIVE', 'DELETED', 'FAILED_DELETION'
    deleted_at TIMESTAMPTZ
);

-- 4. LMS EVALUATIONS
CREATE TABLE IF NOT EXISTS lms_evaluations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    task_id UUID NOT NULL UNIQUE REFERENCES lms_tasks(id) ON DELETE CASCADE,
    evaluation_title VARCHAR(255) NOT NULL,
    maximum_marks NUMERIC(6, 2) NOT NULL DEFAULT 100.00,
    pass_marks NUMERIC(6, 2) DEFAULT 35.00,
    evaluation_category VARCHAR(100),
    evaluation_date DATE,
    include_in_results BOOLEAN NOT NULL DEFAULT FALSE,
    result_component_id UUID,
    status VARCHAR(50) NOT NULL DEFAULT 'PENDING', -- 'PENDING', 'IN_PROGRESS', 'COMPLETED'
    created_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 5. LMS EVALUATION MARKS (Separated marks storage)
CREATE TABLE IF NOT EXISTS lms_evaluation_marks (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    evaluation_id UUID NOT NULL REFERENCES lms_evaluations(id) ON DELETE CASCADE,
    student_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    marks_obtained NUMERIC(6, 2),
    is_absent BOOLEAN NOT NULL DEFAULT FALSE,
    question_marks JSONB NOT NULL DEFAULT '[]'::jsonb,
    remarks TEXT,
    entered_by UUID REFERENCES users(id) ON DELETE SET NULL,
    entered_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT uq_lms_evaluation_student UNIQUE (evaluation_id, student_id)
);

-- 6. LMS TASK AUDIT LOGS
CREATE TABLE IF NOT EXISTS lms_task_audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    task_id UUID NOT NULL REFERENCES lms_tasks(id) ON DELETE CASCADE,
    actor_id UUID REFERENCES users(id) ON DELETE SET NULL,
    action VARCHAR(100) NOT NULL, -- 'CREATED', 'UPDATED', 'PUBLISHED', 'DEADLINE_CHANGED', 'ATTACHMENT_ADDED', 'ATTACHMENT_REMOVED', 'EVALUATION_CONFIGURED', 'MARKS_ENTERED', 'MARKS_UPDATED', 'CANCELLED'
    details JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- 7. PERFORMANCE & AUDIT INDEXES
CREATE INDEX IF NOT EXISTS idx_lms_tasks_school_year ON lms_tasks(school_id, academic_year_id);
CREATE INDEX IF NOT EXISTS idx_lms_tasks_section ON lms_tasks(school_id, class_section_id);
CREATE INDEX IF NOT EXISTS idx_lms_tasks_subject ON lms_tasks(school_id, subject_id);
CREATE INDEX IF NOT EXISTS idx_lms_tasks_attendance_session ON lms_tasks(attendance_session_id);
CREATE INDEX IF NOT EXISTS idx_lms_tasks_created_by ON lms_tasks(school_id, created_by);
CREATE INDEX IF NOT EXISTS idx_lms_tasks_status ON lms_tasks(school_id, status);
CREATE INDEX IF NOT EXISTS idx_lms_tasks_deadline ON lms_tasks(school_id, deadline_at);

CREATE INDEX IF NOT EXISTS idx_lms_recipients_student ON lms_task_recipients(school_id, student_id);
CREATE INDEX IF NOT EXISTS idx_lms_recipients_task ON lms_task_recipients(task_id);

CREATE INDEX IF NOT EXISTS idx_lms_attachments_task ON lms_task_attachments(task_id);
CREATE INDEX IF NOT EXISTS idx_lms_attachments_expiry ON lms_task_attachments(expires_at, deletion_status)
    WHERE attachment_type = 'TEMPORARY_UPLOAD';

CREATE INDEX IF NOT EXISTS idx_lms_eval_task ON lms_evaluations(task_id);
CREATE INDEX IF NOT EXISTS idx_lms_eval_marks_eval ON lms_evaluation_marks(evaluation_id);
CREATE INDEX IF NOT EXISTS idx_lms_eval_marks_student ON lms_evaluation_marks(school_id, student_id);

CREATE INDEX IF NOT EXISTS idx_lms_audit_task ON lms_task_audit_logs(task_id, created_at DESC);
