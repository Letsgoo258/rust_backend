-- Migration: 20261110000001_optimize_users_list_lateral_and_indexes.sql
-- Optimizes users list query performance, lateral role aggregation, and foreign key joins

-- 1. Composite index for school_id + created_at DESC (primary filter & sort for users list)
CREATE INDEX IF NOT EXISTS idx_users_school_created
  ON users (school_id, created_at DESC);

-- 2. Enrollments lookup by student_id and status
CREATE INDEX IF NOT EXISTS idx_enrollments_student_status
  ON enrollments (student_id, status);

-- 3. User roles lookup by user_id
CREATE INDEX IF NOT EXISTS idx_user_roles_user
  ON user_roles (user_id);

-- 4. Student guardians lookup by student_id with primary ordering
CREATE INDEX IF NOT EXISTS idx_student_guardians_student
  ON student_guardians (student_id, is_primary DESC, is_emergency_contact DESC);

-- 5. Staff join lookup by user_id
CREATE INDEX IF NOT EXISTS idx_staff_user_id
  ON staff (user_id);
