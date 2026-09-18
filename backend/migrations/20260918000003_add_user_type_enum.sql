-- Add System-Level Role Enum
CREATE TYPE system_role_type AS ENUM (
    'SUPER_ADMIN',  -- Eravaya platform administrators
    'SCHOOL_ADMIN', -- Principal / School Owners
    'STAFF',        -- General staff (HR, Accounts, etc)
    'TEACHER',      -- Academic teachers
    'STUDENT',      -- Enrolled students
    'PARENT'        -- Guardians
);

-- Add the user_type column to the users table
ALTER TABLE users ADD COLUMN user_type system_role_type NOT NULL DEFAULT 'STUDENT';
