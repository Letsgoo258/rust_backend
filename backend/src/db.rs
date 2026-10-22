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
                                                                                            // Increase slow statement threshold from 1s to 5s to avoid noisy logs in Supabase
    let connect_options = connect_options
        .log_slow_statements(tracing::log::LevelFilter::Warn, Duration::from_secs(5));

    let pool = PgPoolOptions::new()
        .max_connections(50)
        .acquire_timeout(Duration::from_secs(30))
        .connect_with(connect_options)
        .await?;

    tracing::info!("Running database migrations...");
    if let Err(e) = sqlx::migrate!("./migrations").run(&pool).await {
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

            -- Performance tweak: Make frontend_telemetry unlogged to avoid WAL overhead for high-volume analytics
            ALTER TABLE frontend_telemetry SET UNLOGGED;

            -- Performance tweak: Functional index for admission number lookups
            CREATE INDEX IF NOT EXISTS idx_users_upper_admission_number ON users (school_id, UPPER(admission_number));

            -- Ensure class_section_id column exists on users table
            ALTER TABLE users ADD COLUMN IF NOT EXISTS class_section_id UUID REFERENCES class_sections(id) ON DELETE SET NULL;

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
        EXCEPTION WHEN OTHERS THEN
            NULL;
        END $$;
        "#
    ).execute(&pool).await;

    // Ensure role_capabilities schema is created
    crate::utils::capabilities::CapabilityController::ensure_schema(&pool).await?;

    Ok(pool)
}
