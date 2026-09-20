-- 20260918000001_initial_schema.sql
-- ERAVAYA ERP Initial Schema
-- Enforcing strict native types, UUID identifiers, and tenant isolation.

-- 1. ENUMS
CREATE TYPE school_status AS ENUM ('ACTIVE', 'INACTIVE', 'SUSPENDED');
CREATE TYPE user_status AS ENUM ('ACTIVE', 'INACTIVE', 'LOCKED');

-- 2. SCHOOLS (Top-level Tenant)
CREATE TABLE schools (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL,
    code VARCHAR(50) UNIQUE NOT NULL,
    short_name VARCHAR(100),
    email VARCHAR(255),
    phone VARCHAR(50),
    website VARCHAR(255),
    address_line_1 TEXT,
    address_line_2 TEXT,
    city VARCHAR(100),
    district VARCHAR(100),
    state VARCHAR(100),
    country VARCHAR(100),
    postal_code VARCHAR(20),
    timezone VARCHAR(50) DEFAULT 'UTC',
    logo_file_id UUID, -- References file storage later
    status school_status DEFAULT 'ACTIVE',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 3. SCHOOL BRANCHES
CREATE TABLE school_branches (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    code VARCHAR(50) NOT NULL,
    address TEXT,
    phone VARCHAR(50),
    email VARCHAR(255),
    status school_status DEFAULT 'ACTIVE',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(school_id, code)
);

-- 4. USERS (Identity & Authentication)
CREATE TABLE users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    branch_id UUID REFERENCES school_branches(id),
    username VARCHAR(100) NOT NULL,
    email VARCHAR(255),
    phone VARCHAR(50),
    password_hash VARCHAR(255) NOT NULL,
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    display_name VARCHAR(255),
    status user_status DEFAULT 'ACTIVE',
    last_login_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(school_id, username),
    UNIQUE(school_id, email)
);

-- 5. ROLES (RBAC)
CREATE TABLE roles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    code VARCHAR(50) NOT NULL,
    description TEXT,
    is_system_role BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(school_id, code)
);

-- 6. PERMISSIONS (RBAC)
CREATE TABLE permissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code VARCHAR(100) UNIQUE NOT NULL, -- e.g., 'student.read', 'attendance.create'
    name VARCHAR(255) NOT NULL,
    description TEXT
);

-- 7. ROLE PERMISSIONS (Many-to-Many)
CREATE TABLE role_permissions (
    role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permission_id UUID NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    PRIMARY KEY (role_id, permission_id)
);

-- 8. USER ROLES (Many-to-Many)
CREATE TABLE user_roles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(user_id, role_id)
);

-- 9. USER SESSIONS (For tower-sessions-sqlx-store or custom management)
CREATE TABLE user_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash VARCHAR(255) NOT NULL UNIQUE,
    ip_address VARCHAR(45),
    user_agent TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at TIMESTAMPTZ
);

-- Trigger for updated_at
CREATE OR REPLACE FUNCTION trigger_set_timestamp()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Apply updated_at triggers
CREATE TRIGGER set_timestamp_schools
BEFORE UPDATE ON schools
FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

CREATE TRIGGER set_timestamp_users
BEFORE UPDATE ON users
FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

CREATE TRIGGER set_timestamp_roles
BEFORE UPDATE ON roles
FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();



-- 20260918000002_academic_foundation.sql
-- ERAVAYA ERP - Academic Foundation Schema

-- 1. ACADEMIC YEARS
CREATE TABLE academic_years (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL, -- e.g., "2026-27"
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    status VARCHAR(50) DEFAULT 'UPCOMING', -- UPCOMING, ACTIVE, COMPLETED
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(school_id, name)
);

-- 2. ACADEMIC TERMS
CREATE TABLE academic_terms (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    code VARCHAR(50) NOT NULL,
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    sequence INT NOT NULL,
    status VARCHAR(50) DEFAULT 'ACTIVE',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(academic_year_id, code)
);

-- 3. ACADEMIC CALENDAR
CREATE TABLE academic_calendar (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    type VARCHAR(50) NOT NULL, -- HOLIDAY, EXAM, EVENT, WORKING_DAY
    title VARCHAR(255) NOT NULL,
    description TEXT,
    is_working_day BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 4. CLASSES
CREATE TABLE classes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL, -- e.g., "Class 8"
    code VARCHAR(50) NOT NULL,
    sequence INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(school_id, code)
);

-- 5. SECTIONS
CREATE TABLE sections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL, -- e.g., "A", "B"
    code VARCHAR(50) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(school_id, code)
);

-- 6. ROOMS
CREATE TABLE rooms (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    branch_id UUID REFERENCES school_branches(id),
    name VARCHAR(100) NOT NULL,
    code VARCHAR(50) NOT NULL,
    capacity INT,
    room_type VARCHAR(50),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(school_id, code)
);

-- 7. CLASS SECTIONS (Grouping for a specific academic year)
CREATE TABLE class_sections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    class_id UUID NOT NULL REFERENCES classes(id) ON DELETE CASCADE,
    section_id UUID NOT NULL REFERENCES sections(id) ON DELETE CASCADE,
    class_teacher_staff_id UUID, -- References staff(id) eventually
    capacity INT,
    room_id UUID REFERENCES rooms(id),
    status VARCHAR(50) DEFAULT 'ACTIVE',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(academic_year_id, class_id, section_id)
);

-- 8. SUBJECTS
CREATE TABLE subjects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    code VARCHAR(50) NOT NULL,
    subject_type VARCHAR(50), -- e.g., CORE, ELECTIVE
    description TEXT,
    status VARCHAR(50) DEFAULT 'ACTIVE',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(school_id, code)
);

-- Apply updated_at triggers
CREATE TRIGGER set_timestamp_academic_years BEFORE UPDATE ON academic_years FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
CREATE TRIGGER set_timestamp_academic_terms BEFORE UPDATE ON academic_terms FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
CREATE TRIGGER set_timestamp_classes BEFORE UPDATE ON classes FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
CREATE TRIGGER set_timestamp_sections BEFORE UPDATE ON sections FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
CREATE TRIGGER set_timestamp_rooms BEFORE UPDATE ON rooms FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
CREATE TRIGGER set_timestamp_class_sections BEFORE UPDATE ON class_sections FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
CREATE TRIGGER set_timestamp_subjects BEFORE UPDATE ON subjects FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();


-- 20260918000003_add_user_type_enum.sql
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


-- 20260918000004_seed_permissions.sql
-- Insert the core system permissions globally
INSERT INTO permissions (code, name, description) VALUES
('USERS_CREATE', 'Create Users', 'Allows creating new users in the school'),
('USERS_VIEW', 'View Users', 'Allows viewing users in the school'),
('USERS_EDIT', 'Edit Users', 'Allows editing users in the school'),
('USERS_DELETE', 'Delete Users', 'Allows deleting users in the school'),
('ACADEMICS_MANAGE', 'Manage Academics', 'Allows managing classes, sections, and subjects'),
('ATTENDANCE_MARK', 'Mark Attendance', 'Allows marking daily attendance'),
('FEES_MANAGE', 'Manage Fees', 'Allows generating invoices and recording payments')
ON CONFLICT (code) DO NOTHING;


-- 20260918000005_core_sis.sql
CREATE TABLE students (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    user_id UUID UNIQUE REFERENCES users(id) ON DELETE SET NULL,
    admission_number VARCHAR(50) NOT NULL,
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    date_of_birth DATE NOT NULL,
    gender VARCHAR(20),
    blood_group VARCHAR(10),
    status VARCHAR(20) DEFAULT 'ACTIVE',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(school_id, admission_number)
);

CREATE TABLE guardians (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    user_id UUID UNIQUE REFERENCES users(id) ON DELETE SET NULL,
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    email VARCHAR(255),
    phone VARCHAR(50),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE student_guardians (
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    guardian_id UUID NOT NULL REFERENCES guardians(id) ON DELETE CASCADE,
    relationship VARCHAR(50) NOT NULL,
    is_primary BOOLEAN DEFAULT false,
    is_emergency_contact BOOLEAN DEFAULT false,
    PRIMARY KEY (student_id, guardian_id)
);

CREATE TABLE enrollments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    student_id UUID NOT NULL REFERENCES students(id) ON DELETE CASCADE,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE RESTRICT,
    class_section_id UUID NOT NULL REFERENCES class_sections(id) ON DELETE RESTRICT,
    roll_number VARCHAR(50),
    enrollment_date DATE NOT NULL DEFAULT CURRENT_DATE,
    status VARCHAR(20) DEFAULT 'ACTIVE',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(student_id, academic_year_id)
);


-- 20260919000001_add_user_verification.sql
ALTER TABLE users ADD COLUMN email_verified BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE users ADD COLUMN phone_verified BOOLEAN NOT NULL DEFAULT false;


-- 20260919000002_auto_verify_super_admins.sql
UPDATE users SET email_verified = true, phone_verified = true WHERE user_type = 'SUPER_ADMIN';


-- 20260919000003_add_school_pending_status.sql
ALTER TYPE school_status ADD VALUE 'PENDING_SETUP';


-- 20260919000004_multiple_roles_and_indexes.sql
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


-- 20260919000005_add_audit_and_telemetry.sql
-- Audit Logs for critical actions (Login, Add Staff, Remove Staff, etc.)
CREATE TABLE audit_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID REFERENCES schools(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    action VARCHAR(255) NOT NULL,
    resource_type VARCHAR(100),
    resource_id UUID,
    details JSONB,
    ip_address VARCHAR(45),
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_audit_logs_school_id ON audit_logs(school_id);
CREATE INDEX idx_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX idx_audit_logs_action ON audit_logs(action);

-- Frontend Telemetry for analytics (App Opens, Errors, Session Duration, Clicks)
CREATE TABLE frontend_telemetry (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID REFERENCES schools(id) ON DELETE CASCADE,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    session_id VARCHAR(255) NOT NULL,
    event_type VARCHAR(100) NOT NULL, -- e.g., 'APP_OPEN', 'ERROR', 'CLICK', 'SESSION_DURATION'
    url TEXT,
    component VARCHAR(255),
    error_message TEXT,
    error_stack TEXT,
    duration_ms INT,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX idx_telemetry_school_id ON frontend_telemetry(school_id);
CREATE INDEX idx_telemetry_session ON frontend_telemetry(session_id);
CREATE INDEX idx_telemetry_event_type ON frontend_telemetry(event_type);


-- 20260919000006_admin_action_otps.sql
CREATE TABLE admin_action_otps (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    action_type VARCHAR NOT NULL,
    target_id UUID,
    otp_code VARCHAR NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);


-- 20260920000001_add_comprehensive_indian_school_roles.sql
-- Add comprehensive Indian school roles to system_role_type
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'VICE_PRINCIPAL';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'HEADMASTER';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'CLASS_TEACHER';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'SUBJECT_COORDINATOR';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'EXAM_COORDINATOR';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'LIBRARIAN';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'LAB_ASSISTANT';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'COUNSELOR';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'PT_TEACHER';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'RECEPTIONIST';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'HR_MANAGER';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'ACCOUNTANT';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'CLERK';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'TRANSPORT_MANAGER';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'DRIVER';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'BUS_ATTENDANT';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'CANTEEN_MANAGER';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'CANTEEN_STAFF';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'SECURITY_HEAD';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'SECURITY_GUARD';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'PEON';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'HOUSEKEEPING';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'SUPPORT_STAFF';
ALTER TYPE system_role_type ADD VALUE IF NOT EXISTS 'ALUMNI';


-- 20260920000002_user_csv_tracking.sql
ALTER TABLE users ADD COLUMN imported_via_csv BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE users ADD COLUMN updated_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL;

-- Automatically clean up audit logs older than 30 days to save space and maintain compliance
-- We will create a small function to do this, though typically it's called by a cron job or application logic


-- 20260920000003_add_notifications.sql
CREATE TYPE notification_type AS ENUM ('EMAIL', 'SMS', 'PUSH');
CREATE TYPE notification_status AS ENUM ('PENDING', 'SENT', 'FAILED');

CREATE TABLE notifications (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    message TEXT NOT NULL,
    notification_type notification_type NOT NULL,
    status notification_status NOT NULL DEFAULT 'PENDING',
    priority INT NOT NULL DEFAULT 2, -- 1 = Critical, 2 = Normal, 3 = Low
    retry_count INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_notifications_status_priority ON notifications(status, priority, created_at);


