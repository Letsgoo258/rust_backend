-- Migration: 20261101000003_optimize_users_query_indexes.sql
-- Optimizes list_users query, sorting, and foreign key join lookups

-- 1. Composite index for school_id + created_at DESC (accelerates ORDER BY u.created_at DESC)
CREATE INDEX IF NOT EXISTS idx_users_school_created
ON users (school_id, created_at DESC);

-- 2. Foreign key join indexes for student_guardians, user_roles, and users
CREATE INDEX IF NOT EXISTS idx_student_guardians_guardian_id
ON student_guardians (guardian_id);

CREATE INDEX IF NOT EXISTS idx_user_roles_role_id
ON user_roles (role_id);

CREATE INDEX IF NOT EXISTS idx_users_class_section_id
ON users (class_section_id) WHERE class_section_id IS NOT NULL;
