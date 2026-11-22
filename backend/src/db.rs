use crate::config::AppConfig;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    ConnectOptions, PgPool,
};
use std::str::FromStr;
use std::time::Duration;

pub async fn init_pool(config: &AppConfig) -> Result<PgPool, sqlx::Error> {
    let mut db_url = config.database_url.clone();
    if !db_url.contains("sslrootcert") {
        let cert_path = if std::path::Path::new("ca.pem").exists() {
            "ca.pem"
        } else if std::path::Path::new("backend/ca.pem").exists() {
            "backend/ca.pem"
        } else {
            "ca.pem"
        };
        db_url = format!("{}&sslrootcert={}", db_url, cert_path);
    }

    let connect_options = PgConnectOptions::from_str(&db_url)?.statement_cache_capacity(0); // Fix for PgBouncer/Supabase introspection overhead
                                                                                            // Log any queries taking > 1s as warnings to catch bottlenecks early
    let connect_options = connect_options
        .log_slow_statements(tracing::log::LevelFilter::Warn, Duration::from_secs(1));

    let max_connections: u32 = std::env::var("DATABASE_MAX_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4);
    let min_connections: u32 = std::env::var("DATABASE_MIN_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let idle_timeout_secs: u64 = std::env::var("DATABASE_IDLE_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);

    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .min_connections(min_connections)
        .idle_timeout(Duration::from_secs(idle_timeout_secs))
        .acquire_slow_threshold(Duration::from_secs(2))
        .acquire_timeout(Duration::from_secs(15))
        .connect_with(connect_options)
        .await?;

    tracing::info!("Running database migrations...");
    let mut migrator = sqlx::migrate!("./migrations");
    migrator.set_locking(false);
    if let Err(e) = migrator.run(&pool).await {
        tracing::warn!(
            "Migration check warning: {:?}. Proceeding with connected database pool.",
            e
        );
    } else {
        tracing::info!("Database migrations applied successfully.");
    }

    // Direct fallback ensure for enrollments foreign key cascades
    let _ = sqlx::query(
        r#"
        DO $$
        BEGIN
            ALTER TABLE enrollments DROP CONSTRAINT IF EXISTS enrollments_academic_year_id_fkey;
            ALTER TABLE enrollments ADD CONSTRAINT enrollments_academic_year_id_fkey
                FOREIGN KEY (academic_year_id) REFERENCES academic_years(id) ON DELETE CASCADE ON UPDATE CASCADE;

            ALTER TABLE enrollments DROP CONSTRAINT IF EXISTS enrollments_class_section_id_fkey;
            ALTER TABLE enrollments ADD CONSTRAINT enrollments_class_section_id_fkey
                FOREIGN KEY (class_section_id) REFERENCES class_sections(id) ON DELETE CASCADE ON UPDATE CASCADE;

            -- Allow unassigned enrollments for admitted students prior to class division
            ALTER TABLE enrollments ALTER COLUMN class_section_id DROP NOT NULL;

            -- Performance tweak: Make frontend_telemetry unlogged to avoid WAL overhead for high-volume analytics
            ALTER TABLE frontend_telemetry SET UNLOGGED;

            -- Performance tweak: Functional index for admission number lookups
            CREATE INDEX IF NOT EXISTS idx_users_upper_admission_number ON users (school_id, UPPER(admission_number));

            -- Ensure class_section_id column exists on users table
            ALTER TABLE users ADD COLUMN IF NOT EXISTS class_section_id UUID REFERENCES class_sections(id) ON DELETE SET NULL;

            -- Ensure caste_category and custom_values exist on users table
            ALTER TABLE users ADD COLUMN IF NOT EXISTS caste_category VARCHAR(100);
            ALTER TABLE users ADD COLUMN IF NOT EXISTS custom_values JSONB DEFAULT '{}'::jsonb;
            CREATE INDEX IF NOT EXISTS idx_users_caste_category ON users (school_id, caste_category);

            -- Ensure user_custom_values table exists
            CREATE TABLE IF NOT EXISTS user_custom_values (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
                user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                field_id UUID NOT NULL REFERENCES custom_fields(id) ON DELETE CASCADE,
                value TEXT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                UNIQUE(user_id, field_id)
            );
            CREATE INDEX IF NOT EXISTS idx_user_custom_values_user ON user_custom_values (user_id);
            CREATE INDEX IF NOT EXISTS idx_user_custom_values_field ON user_custom_values (field_id);

            -- Ensure report_sequences and issued_reports tables exist
            CREATE TABLE IF NOT EXISTS report_sequences (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
                report_type VARCHAR(50) NOT NULL,
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
                status VARCHAR(30) NOT NULL DEFAULT 'ISSUED',
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

            -- Cancellation audit columns for issued_reports
            ALTER TABLE issued_reports ADD COLUMN IF NOT EXISTS cancelled_by UUID REFERENCES users(id) ON DELETE SET NULL;
            ALTER TABLE issued_reports ADD COLUMN IF NOT EXISTS cancelled_by_name VARCHAR(255);
            ALTER TABLE issued_reports ADD COLUMN IF NOT EXISTS cancelled_at TIMESTAMPTZ;
            ALTER TABLE issued_reports ADD COLUMN IF NOT EXISTS cancellation_reason TEXT;

            -- Backfill existing CANCELLED reports
            UPDATE issued_reports
            SET cancelled_at = COALESCE(cancelled_at, updated_at),
                cancelled_by_name = COALESCE(cancelled_by_name, issuer_name, 'System Administrator'),
                cancellation_reason = COALESCE(cancellation_reason, remarks, 'Cancelled redundant duplicate issuance')
            WHERE status = 'CANCELLED' AND (cancelled_at IS NULL OR cancellation_reason IS NULL);

            -- Ensure users status supports full student lifecycle status strings (including TC_ISSUED)
            ALTER TABLE users ALTER COLUMN status TYPE VARCHAR(50) USING status::TEXT;
            ALTER TABLE users ALTER COLUMN status SET DEFAULT 'ACTIVE';

            -- Database safeguard: Ensure any existing users missing entries in user_roles get assigned appropriate roles
            INSERT INTO user_roles (user_id, role_id)
            SELECT u.id, r.id
            FROM users u
            JOIN roles r ON r.school_id = u.school_id AND r.code = CASE
                WHEN u.admission_number IS NOT NULL THEN 'STUDENT'
                WHEN EXISTS (SELECT 1 FROM guardians g WHERE g.user_id = u.id) THEN 'PARENT'
                WHEN EXISTS (SELECT 1 FROM staff s WHERE s.user_id = u.id) THEN 'TEACHER'
                ELSE 'SCHOOL_ADMIN'
            END
            WHERE NOT EXISTS (SELECT 1 FROM user_roles ur WHERE ur.user_id = u.id)
            ON CONFLICT DO NOTHING;

            -- Performance tweak: Drop redundant overlapping indices
            DROP INDEX IF EXISTS idx_students_upper_admission_number;
            DROP INDEX IF EXISTS idx_students_school_id;
            DROP INDEX IF EXISTS idx_users_school_id;

            -- Ensure subject_types table exists
            CREATE TABLE IF NOT EXISTS subject_types (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
                name VARCHAR(100) NOT NULL,
                code VARCHAR(50) NOT NULL,
                system_subject_type VARCHAR(50) NOT NULL DEFAULT 'Core',
                is_elective BOOLEAN NOT NULL DEFAULT FALSE,
                is_mandatory BOOLEAN NOT NULL DEFAULT TRUE,
                description TEXT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                UNIQUE(school_id, code)
            );

            ALTER TABLE subjects ADD COLUMN IF NOT EXISTS system_subject_type VARCHAR(50) NOT NULL DEFAULT 'Core';
            ALTER TABLE subjects ADD COLUMN IF NOT EXISTS subject_type_id UUID REFERENCES subject_types(id) ON DELETE SET NULL;
            ALTER TABLE subjects ADD COLUMN IF NOT EXISTS color VARCHAR(20) DEFAULT '#4F46E5';
            ALTER TABLE subjects ADD COLUMN IF NOT EXISTS department VARCHAR(100);
            ALTER TABLE subjects ADD COLUMN IF NOT EXISTS class_id UUID REFERENCES classes(id) ON DELETE SET NULL;

            -- Ensure settings modules tables exist
            CREATE TABLE IF NOT EXISTS assets (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
                name VARCHAR(255) NOT NULL,
                category VARCHAR(100),
                asset_code VARCHAR(100),
                purchase_date DATE,
                status VARCHAR(50) DEFAULT 'ACTIVE',
                room_id UUID REFERENCES rooms(id) ON DELETE SET NULL,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS school_timings (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
                shift_name VARCHAR(100) NOT NULL,
                start_time TIME NOT NULL,
                end_time TIME NOT NULL,
                late_mark_time TIME,
                half_day_time TIME,
                is_default BOOLEAN DEFAULT false,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS background_jobs (
                id UUID PRIMARY KEY,
                school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
                user_id UUID NOT NULL,
                job_type VARCHAR(100) NOT NULL,
                status VARCHAR(50) NOT NULL DEFAULT 'PROCESSING',
                total_rows INT NOT NULL DEFAULT 0,
                processed_rows INT NOT NULL DEFAULT 0,
                success_count INT NOT NULL DEFAULT 0,
                failed_count INT NOT NULL DEFAULT 0,
                failed_csv TEXT,
                error TEXT,
                payload_json JSONB,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS data_change_tickets (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
                user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                requested_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
                ticket_number VARCHAR(50) NOT NULL,
                ticket_type VARCHAR(50) NOT NULL DEFAULT 'CREDENTIAL_CHANGE',
                status VARCHAR(50) NOT NULL DEFAULT 'PENDING',
                field_name VARCHAR(100) NOT NULL,
                old_value TEXT,
                new_value TEXT NOT NULL,
                reason TEXT NOT NULL,
                priority VARCHAR(20) NOT NULL DEFAULT 'STANDARD',
                reviewer_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
                reviewer_notes TEXT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_data_change_tickets_user ON data_change_tickets(user_id);
            CREATE INDEX IF NOT EXISTS idx_data_change_tickets_school ON data_change_tickets(school_id);
            CREATE INDEX IF NOT EXISTS idx_data_change_tickets_status ON data_change_tickets(status);

            -- Print queue table with 24-hour expiration window
            CREATE TABLE IF NOT EXISTS print_queue (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
                user_id UUID REFERENCES users(id) ON DELETE SET NULL,
                uploaded_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                document_type VARCHAR(50) NOT NULL,
                title VARCHAR(255) NOT NULL,
                recipient_name VARCHAR(255) NOT NULL,
                recipient_phone VARCHAR(50),
                recipient_admission_number VARCHAR(100),
                file_url TEXT,
                file_name VARCHAR(255),
                file_size_bytes BIGINT DEFAULT 0,
                copies INT DEFAULT 1,
                status VARCHAR(30) NOT NULL DEFAULT 'PENDING',
                options JSONB NOT NULL DEFAULT '{}'::jsonb,
                metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
                created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
                expires_at TIMESTAMPTZ NOT NULL DEFAULT (now() + interval '24 hours'),
                printed_at TIMESTAMPTZ,
                printed_by UUID REFERENCES users(id) ON DELETE SET NULL
            );
            CREATE INDEX IF NOT EXISTS idx_print_queue_school_status ON print_queue(school_id, status, expires_at);
            CREATE INDEX IF NOT EXISTS idx_print_queue_recipient ON print_queue(school_id, recipient_phone, recipient_admission_number);
            CREATE INDEX IF NOT EXISTS idx_print_queue_uploaded_by ON print_queue(uploaded_by, expires_at);
            CREATE INDEX IF NOT EXISTS idx_print_queue_expires ON print_queue(expires_at);

            -- Ensure LMS tables exist
            CREATE TABLE IF NOT EXISTS lms_tasks (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
                academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
                title VARCHAR(255) NOT NULL,
                description TEXT NOT NULL,
                special_message TEXT,
                task_type VARCHAR(50) NOT NULL DEFAULT 'HOMEWORK',
                class_section_id UUID NOT NULL REFERENCES class_sections(id) ON DELETE CASCADE,
                subject_id UUID REFERENCES subjects(id) ON DELETE SET NULL,
                teaching_group_id UUID,
                attendance_session_id UUID REFERENCES attendance_sessions(id) ON DELETE RESTRICT,
                created_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                assigned_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                deadline_at TIMESTAMPTZ,
                status VARCHAR(50) NOT NULL DEFAULT 'DRAFT',
                add_to_calendar BOOLEAN NOT NULL DEFAULT FALSE,
                calendar_event_id UUID REFERENCES academic_calendar(id) ON DELETE SET NULL,
                notify_whatsapp BOOLEAN NOT NULL DEFAULT FALSE,
                notify_email BOOLEAN NOT NULL DEFAULT FALSE,
                is_evaluation BOOLEAN NOT NULL DEFAULT FALSE,
                published_at TIMESTAMPTZ,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS lms_task_recipients (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
                task_id UUID NOT NULL REFERENCES lms_tasks(id) ON DELETE CASCADE,
                student_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                enrollment_id UUID REFERENCES enrollments(id) ON DELETE SET NULL,
                recipient_status VARCHAR(50) NOT NULL DEFAULT 'ASSIGNED',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                CONSTRAINT uq_lms_task_recipient UNIQUE (task_id, student_id)
            );

            CREATE TABLE IF NOT EXISTS lms_task_attachments (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
                task_id UUID NOT NULL REFERENCES lms_tasks(id) ON DELETE CASCADE,
                attachment_type VARCHAR(50) NOT NULL,
                original_filename VARCHAR(255),
                mime_type VARCHAR(100),
                file_size BIGINT,
                imagekit_file_id VARCHAR(255),
                file_url TEXT NOT NULL,
                google_drive_url TEXT,
                uploaded_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                uploaded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                expires_at TIMESTAMPTZ,
                deletion_status VARCHAR(50) NOT NULL DEFAULT 'ACTIVE',
                deleted_at TIMESTAMPTZ
            );

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
                status VARCHAR(50) NOT NULL DEFAULT 'PENDING',
                created_by UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

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

            CREATE TABLE IF NOT EXISTS lms_task_audit_logs (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
                task_id UUID NOT NULL REFERENCES lms_tasks(id) ON DELETE CASCADE,
                actor_id UUID REFERENCES users(id) ON DELETE SET NULL,
                action VARCHAR(100) NOT NULL,
                details JSONB NOT NULL DEFAULT '{}'::jsonb,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

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
            CREATE INDEX IF NOT EXISTS idx_lms_eval_task ON lms_evaluations(task_id);
            CREATE INDEX IF NOT EXISTS idx_lms_eval_marks_eval ON lms_evaluation_marks(evaluation_id);
            CREATE INDEX IF NOT EXISTS idx_lms_eval_marks_student ON lms_evaluation_marks(school_id, student_id);
            CREATE INDEX IF NOT EXISTS idx_lms_audit_task ON lms_task_audit_logs(task_id, created_at DESC);
        EXCEPTION WHEN OTHERS THEN
            NULL;
        END $$;
        "#
    ).execute(&pool).await;

    // Ensure role_capabilities schema is created
    crate::utils::capabilities::CapabilityController::ensure_schema(&pool).await?;

    // Ensure whatsapp tables and outbox schema are created
    let _ = crate::utils::whatsapp::ensure_schema(&pool).await;

    Ok(pool)
}
