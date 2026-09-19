-- 1. Indexing on school_id for hard segregation performance
CREATE INDEX IF NOT EXISTS idx_users_school_id ON users(school_id);
CREATE INDEX IF NOT EXISTS idx_students_school_id ON students(school_id);
CREATE INDEX IF NOT EXISTS idx_guardians_school_id ON guardians(school_id);

-- 2. Convert user_type to an array of roles to support multiple roles per person
ALTER TABLE users 
  ALTER COLUMN user_type DROP DEFAULT,
  ALTER COLUMN user_type TYPE system_role_type[] USING ARRAY[user_type]::system_role_type[],
  ALTER COLUMN user_type SET DEFAULT '{STUDENT}';

-- Rename the column to 'roles' for clarity
ALTER TABLE users RENAME COLUMN user_type TO roles;
