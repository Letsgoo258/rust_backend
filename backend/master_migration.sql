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
    admission_number VARCHAR(50),
    date_of_birth DATE,
    gender VARCHAR(20),
    blood_group VARCHAR(10),
    status user_status DEFAULT 'ACTIVE',
    last_login_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(school_id, username),
    UNIQUE(school_id, email),
    UNIQUE(school_id, admission_number)
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

CREATE TABLE subject_types (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    code VARCHAR(50) NOT NULL,
    system_subject_type VARCHAR(50) NOT NULL DEFAULT 'Core', -- 'Core', 'Practical', 'Vocational', 'Non-Core'
    is_elective BOOLEAN NOT NULL DEFAULT FALSE,
    is_mandatory BOOLEAN NOT NULL DEFAULT TRUE,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(school_id, code)
);

-- 8. SUBJECTS
CREATE TABLE subjects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    code VARCHAR(50) NOT NULL,
    subject_type VARCHAR(50), -- e.g., CORE, ELECTIVE
    system_subject_type VARCHAR(50) NOT NULL DEFAULT 'Core', -- 'Core', 'Practical', 'Vocational', 'Non-Core'
    subject_type_id UUID REFERENCES subject_types(id) ON DELETE SET NULL,
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
    student_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    guardian_id UUID NOT NULL REFERENCES guardians(id) ON DELETE CASCADE,
    relationship VARCHAR(50) NOT NULL,
    is_primary BOOLEAN DEFAULT false,
    is_emergency_contact BOOLEAN DEFAULT false,
    PRIMARY KEY (student_id, guardian_id)
);

CREATE TABLE enrollments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    student_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
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
-- CREATE INDEX IF NOT EXISTS idx_users_school_id ON users(school_id); -- Redundant: covered by users_school_id_email_key
-- CREATE INDEX IF NOT EXISTS idx_students_school_id ON students(school_id); -- Redundant: covered by students_school_id_admission_number_key
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


-- 20260920000004_add_staff_table.sql
CREATE TABLE staff (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    user_id UUID UNIQUE REFERENCES users(id) ON DELETE SET NULL,
    employee_id VARCHAR(100) NOT NULL,
    department VARCHAR(100),
    designation VARCHAR(100),
    joining_date DATE NOT NULL DEFAULT CURRENT_DATE,
    status VARCHAR(20) DEFAULT 'ACTIVE',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(school_id, employee_id)
);

CREATE TRIGGER set_timestamp_staff BEFORE UPDATE ON staff FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();

-- Some basic permissions for new roles
INSERT INTO permissions (code, name, description) VALUES
('TRANSPORT_MANAGE', 'Manage Transport', 'Allows managing school buses, routes, and drivers'),
('CANTEEN_MANAGE', 'Manage Canteen', 'Allows managing cafeteria inventory and menus'),
('LIBRARY_MANAGE', 'Manage Library', 'Allows managing library books and issuing'),
('SECURITY_LOGS', 'View Security Logs', 'Allows logging and viewing visitor management')
ON CONFLICT (code) DO NOTHING;


-- 20260920000005_add_admissions_table.sql
CREATE TYPE admission_status AS ENUM ('PENDING', 'APPROVED', 'REJECTED', 'ENROLLED');

CREATE TABLE admissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,

    -- Student Details
    student_first_name VARCHAR(100) NOT NULL,
    student_last_name VARCHAR(100) NOT NULL,
    date_of_birth DATE NOT NULL,

    -- Guardian Details
    guardian_first_name VARCHAR(100) NOT NULL,
    guardian_last_name VARCHAR(100) NOT NULL,
    guardian_email VARCHAR(255) NOT NULL,
    guardian_phone VARCHAR(50) NOT NULL,

    -- Workflow
    status admission_status DEFAULT 'PENDING',
    created_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    enrolled_student_user_id UUID REFERENCES users(id) ON DELETE SET NULL,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER set_timestamp_admissions BEFORE UPDATE ON admissions FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();


-- 20260920000006_align_admissions_with_srd.sql
-- Drop the flat admissions table we just created
DROP TABLE IF EXISTS admissions CASCADE;

-- 11.1 admissions
CREATE TABLE admissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    academic_year_id UUID, -- Will link to academic_years later
    application_number VARCHAR(100),
    status admission_status DEFAULT 'PENDING',
    submitted_at TIMESTAMPTZ,
    approved_at TIMESTAMPTZ,
    approved_by UUID REFERENCES users(id) ON DELETE SET NULL,
    rejected_at TIMESTAMPTZ,
    rejected_by UUID REFERENCES users(id) ON DELETE SET NULL,
    rejection_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 11.2 admission_applicants
CREATE TABLE admission_applicants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    admission_id UUID NOT NULL REFERENCES admissions(id) ON DELETE CASCADE,
    first_name VARCHAR(100) NOT NULL,
    middle_name VARCHAR(100),
    last_name VARCHAR(100) NOT NULL,
    date_of_birth DATE NOT NULL,
    gender VARCHAR(20),
    phone VARCHAR(50),
    email VARCHAR(255)
);

-- 11.3 admission_guardians
CREATE TABLE admission_guardians (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    admission_id UUID NOT NULL REFERENCES admissions(id) ON DELETE CASCADE,
    name VARCHAR(255) NOT NULL,
    relationship VARCHAR(50) NOT NULL,
    phone VARCHAR(50) NOT NULL,
    email VARCHAR(255),
    occupation VARCHAR(255)
);

-- 11.4 custom_fields
CREATE TABLE custom_fields (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    entity_type VARCHAR(50) NOT NULL, -- e.g., 'ADMISSION', 'STUDENT'
    field_name VARCHAR(255) NOT NULL,
    field_key VARCHAR(100) NOT NULL,
    field_type VARCHAR(50) NOT NULL, -- e.g., 'TEXT', 'NUMBER', 'DATE', 'SELECT'
    is_required BOOLEAN DEFAULT false,
    options_json JSONB,
    display_order INT DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 11.5 admission_custom_values
CREATE TABLE admission_custom_values (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    admission_id UUID NOT NULL REFERENCES admissions(id) ON DELETE CASCADE,
    field_id UUID NOT NULL REFERENCES custom_fields(id) ON DELETE CASCADE,
    value TEXT,
    UNIQUE(admission_id, field_id)
);

-- 11.6 admission_status_history
CREATE TABLE admission_status_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    admission_id UUID NOT NULL REFERENCES admissions(id) ON DELETE CASCADE,
    from_status admission_status,
    to_status admission_status NOT NULL,
    changed_by UUID REFERENCES users(id) ON DELETE SET NULL,
    reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TRIGGER set_timestamp_admissions BEFORE UPDATE ON admissions FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
CREATE TRIGGER set_timestamp_custom_fields BEFORE UPDATE ON custom_fields FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();


-- 20260920000007_add_password_resets_table.sql
-- Password Resets Schema (SRD Section 8.7)
CREATE TABLE IF NOT EXISTS password_resets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    email VARCHAR(255) NOT NULL,
    token_hash VARCHAR(255) NOT NULL,
    code VARCHAR(10) NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_password_resets_user_id ON password_resets(user_id);
CREATE INDEX IF NOT EXISTS idx_password_resets_email ON password_resets(email);


-- 20260920000008_add_login_history.sql
-- ERAVAYA ERP - Login History & Anomaly Detection Engine

CREATE TABLE IF NOT EXISTS login_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID REFERENCES users(id) ON DELETE CASCADE,
    school_id UUID REFERENCES schools(id) ON DELETE CASCADE,
    username VARCHAR(100) NOT NULL,
    ip_address VARCHAR(45) NOT NULL,
    user_agent TEXT,
    location VARCHAR(255) DEFAULT 'Local/Private Network',
    status VARCHAR(20) NOT NULL DEFAULT 'SUCCESS',
    is_suspicious BOOLEAN DEFAULT false,
    suspicious_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_login_history_user_id ON login_history(user_id);
CREATE INDEX IF NOT EXISTS idx_login_history_created_at ON login_history(created_at);
CREATE INDEX IF NOT EXISTS idx_login_history_is_suspicious ON login_history(is_suspicious);


-- 20260920000009_add_school_operable_and_onboarding_step.sql
-- ERAVAYA ERP - School Operable Flag & Onboarding Step Tracker

ALTER TABLE schools
  ADD COLUMN IF NOT EXISTS operable BOOLEAN DEFAULT false,
  ADD COLUMN IF NOT EXISTS onboarding_step INT DEFAULT 1;

CREATE INDEX IF NOT EXISTS idx_schools_operable ON schools(operable);


-- 20260920000010_default_school_timezone_kolkata.sql
-- Default timezone for schools table to Asia/Kolkata
ALTER TABLE schools ALTER COLUMN timezone SET DEFAULT 'Asia/Kolkata';

UPDATE schools SET timezone = 'Asia/Kolkata' WHERE timezone IS NULL OR timezone = 'UTC';


-- 20260921000001_seed_all_school_roles.sql
-- Seed all 29 system roles into the roles table for every school
INSERT INTO roles (school_id, name, code, description, is_system_role)
SELECT
    s.id,
    r.name,
    r.code,
    r.description,
    true
FROM schools s
CROSS JOIN (
    VALUES
        ('Super Admin', 'SUPER_ADMIN', 'Full system access across all school settings'),
        ('School Admin', 'SCHOOL_ADMIN', 'School level administrator'),
        ('Teacher', 'TEACHER', 'Academic faculty member'),
        ('Student', 'STUDENT', 'Enrolled student'),
        ('Parent', 'PARENT', 'Guardian or parent of a student'),
        ('Vice Principal', 'VICE_PRINCIPAL', 'Deputy head of school'),
        ('Headmaster', 'HEADMASTER', 'Headmaster or principal'),
        ('Class Teacher', 'CLASS_TEACHER', 'Assigned class teacher'),
        ('Subject Coordinator', 'SUBJECT_COORDINATOR', 'Head of subject department'),
        ('Exam Coordinator', 'EXAM_COORDINATOR', 'Controller of examinations'),
        ('Librarian', 'Librarian', 'Library supervisor'),
        ('Lab Assistant', 'Lab Assistant', 'Laboratory assistant'),
        ('Counselor', 'Counselor', 'Student guidance counselor'),
        ('PT Teacher', 'PT Teacher', 'Physical education teacher'),
        ('Receptionist', 'Receptionist', 'Front desk officer'),
        ('HR Manager', 'HR Manager', 'Human resources manager'),
        ('Accountant', 'Accountant', 'Finance and fee manager'),
        ('Clerk', 'Clerk', 'Administrative clerk'),
        ('Transport Manager', 'Transport Manager', 'School transport supervisor'),
        ('Driver', 'Driver', 'School bus driver'),
        ('Bus Attendant', 'Bus Attendant', 'Bus attendant/conductor'),
        ('Canteen Manager', 'Canteen Manager', 'Cafeteria supervisor'),
        ('Canteen Staff', 'Canteen Staff', 'Cafeteria staff'),
        ('Security Head', 'Security Head', 'Chief security officer'),
        ('Security Guard', 'Security Guard', 'Campus security guard'),
        ('Peon', 'Peon', 'Office assistant/peon'),
        ('Housekeeping', 'Housekeeping', 'Maintenance and cleaning staff'),
        ('Support Staff', 'Support Staff', 'General support staff'),
        ('Alumni', 'Alumni', 'Former student alumni')
) AS r(name, code, description)
ON CONFLICT (school_id, code) DO NOTHING;


-- 20260921000002_add_class_sections_updated_by_and_history.sql
ALTER TABLE class_sections
ADD COLUMN IF NOT EXISTS updated_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL;

CREATE TABLE IF NOT EXISTS class_section_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    class_section_id UUID NOT NULL REFERENCES class_sections(id) ON DELETE CASCADE,
    updated_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    change_type VARCHAR(100) NOT NULL,
    old_value TEXT,
    new_value TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_class_section_history_cs_id ON class_section_history(class_section_id);
CREATE INDEX IF NOT EXISTS idx_class_section_history_created_at ON class_section_history(created_at);


-- 20260921000003_fix_school_admin_role_code_typo.sql
UPDATE roles
SET code = TRIM(BOTH '{}' FROM code)
WHERE code LIKE '{%}';


-- 20260921000004_sync_all_users_to_user_roles_ids.sql
INSERT INTO user_roles (user_id, role_id)
SELECT DISTINCT u.id AS user_id, r.id AS role_id
FROM users u
CROSS JOIN LATERAL unnest(u.roles) AS r_code
JOIN roles r ON r.school_id = u.school_id AND r.code = r_code::TEXT
ON CONFLICT (user_id, role_id) DO NOTHING;


-- 20260921000005_user_sessions_and_entity_change_history.sql
CREATE TABLE IF NOT EXISTS entity_change_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    entity_type VARCHAR(100) NOT NULL,
    entity_id UUID NOT NULL,
    action VARCHAR(50) NOT NULL,
    updated_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    before_state JSONB,
    after_state JSONB,
    ip_address VARCHAR(45),
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_entity_change_history_entity ON entity_change_history(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_entity_change_history_school ON entity_change_history(school_id);
CREATE INDEX IF NOT EXISTS idx_entity_change_history_created_at ON entity_change_history(created_at);

ALTER TABLE schools
ADD COLUMN IF NOT EXISTS last_history_id UUID REFERENCES entity_change_history(id) ON DELETE SET NULL,
ADD COLUMN IF NOT EXISTS updated_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE users
ADD COLUMN IF NOT EXISTS last_history_id UUID REFERENCES entity_change_history(id) ON DELETE SET NULL;

ALTER TABLE class_sections
ADD COLUMN IF NOT EXISTS last_history_id UUID REFERENCES entity_change_history(id) ON DELETE SET NULL;

ALTER TABLE academic_years
ADD COLUMN IF NOT EXISTS last_history_id UUID REFERENCES entity_change_history(id) ON DELETE SET NULL,
ADD COLUMN IF NOT EXISTS updated_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE subjects
ADD COLUMN IF NOT EXISTS last_history_id UUID REFERENCES entity_change_history(id) ON DELETE SET NULL,
ADD COLUMN IF NOT EXISTS updated_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL;


-- 20260921000006_fix_enrollments_fkey_cascade.sql
ALTER TABLE enrollments
    DROP CONSTRAINT IF EXISTS enrollments_academic_year_id_fkey,
    ADD CONSTRAINT enrollments_academic_year_id_fkey
    FOREIGN KEY (academic_year_id) REFERENCES academic_years(id) ON DELETE CASCADE ON UPDATE CASCADE;

ALTER TABLE enrollments
    DROP CONSTRAINT IF EXISTS enrollments_class_section_id_fkey,
    ADD CONSTRAINT enrollments_class_section_id_fkey
    FOREIGN KEY (class_section_id) REFERENCES class_sections(id) ON DELETE CASCADE ON UPDATE CASCADE;

-- Assets Management
CREATE TABLE assets (
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

-- School Timings
CREATE TABLE school_timings (
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

-- Background Jobs Persistent Queue (Auto-purged, Zero-clutter)
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

-- Directly link class_section_id on users table (so class & section are linked directly to student users)
ALTER TABLE users ADD COLUMN IF NOT EXISTS class_section_id UUID REFERENCES class_sections(id) ON DELETE SET NULL;
