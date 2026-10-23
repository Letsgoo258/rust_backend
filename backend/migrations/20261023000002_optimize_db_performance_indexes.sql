-- 20261023000002_optimize_db_performance_indexes.sql
-- Optimizations derived from pg_stat_statements: Trigram indexing, covering indexes, and composite filter support

-- 1. Enable pg_trgm extension for fast ILIKE and fuzzy text matching
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- 2. Users table indexes
-- School-scoped chronological sorting for default /dashboard/users view
CREATE INDEX IF NOT EXISTS idx_users_school_created
  ON users (school_id, created_at DESC);

-- School-scoped status filter
CREATE INDEX IF NOT EXISTS idx_users_school_status
  ON users (school_id, status);

-- Partial index for admission number existence scans
CREATE INDEX IF NOT EXISTS idx_users_admission_partial
  ON users (school_id)
  WHERE admission_number IS NOT NULL;

-- Trigram GIN indexes for fast text substring searches
CREATE INDEX IF NOT EXISTS idx_users_username_trgm
  ON users USING gin (username gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_users_display_name_trgm
  ON users USING gin (display_name gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_users_email_trgm
  ON users USING gin (email gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_users_phone_trgm
  ON users USING gin (phone gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_users_admission_trgm
  ON users USING gin (admission_number gin_trgm_ops)
  WHERE admission_number IS NOT NULL;

-- 3. Classes trigram index for class name search
CREATE INDEX IF NOT EXISTS idx_classes_name_trgm
  ON classes USING gin (name gin_trgm_ops);

-- 4. Enrollments & Academic Years lookup indexes
CREATE INDEX IF NOT EXISTS idx_enrollments_student_status
  ON enrollments (student_id, status);

CREATE INDEX IF NOT EXISTS idx_enrollments_class_section_status
  ON enrollments (class_section_id, status);

CREATE INDEX IF NOT EXISTS idx_academic_years_status
  ON academic_years (status);

-- 5. Roles & User Roles covering indexes
CREATE INDEX IF NOT EXISTS idx_user_roles_user_role
  ON user_roles (user_id, role_id);

CREATE INDEX IF NOT EXISTS idx_roles_school_code
  ON roles (school_id, code);

-- 6. Login History anomaly detection & security worker indexes
CREATE INDEX IF NOT EXISTS idx_login_history_user_created
  ON login_history (user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_login_history_user_ip
  ON login_history (user_id, ip_address);

-- 7. Background Jobs poll optimization
CREATE INDEX IF NOT EXISTS idx_background_jobs_updated_at
  ON background_jobs (updated_at DESC);
