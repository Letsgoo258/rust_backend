-- ==============================================================================
-- ERAVAYA ERP - Master Database Schema
-- Note: This is an aggregated, up-to-date version of the schema for reference 
-- and fresh installations. Always ensure it reflects the latest migrations.
-- ==============================================================================

-- ==========================================
-- 1. ENUMS
-- ==========================================
CREATE TYPE school_status AS ENUM ('ACTIVE', 'INACTIVE', 'SUSPENDED');
CREATE TYPE user_status AS ENUM ('ACTIVE', 'INACTIVE', 'LOCKED');

-- ==========================================
-- 2. SCHOOLS & TENANCY
-- ==========================================
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
    logo_file_id UUID,
    status school_status DEFAULT 'ACTIVE',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

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

-- ==========================================
-- 3. IDENTITY, AUTHENTICATION & RBAC
-- ==========================================
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

CREATE TABLE permissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code VARCHAR(100) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT
);

CREATE TABLE role_permissions (
    role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    permission_id UUID NOT NULL REFERENCES permissions(id) ON DELETE CASCADE,
    PRIMARY KEY (role_id, permission_id)
);

CREATE TABLE user_roles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_id UUID NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(user_id, role_id)
);

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

-- ==========================================
-- 4. ACADEMIC FOUNDATION
-- ==========================================
CREATE TABLE academic_years (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    status VARCHAR(50) DEFAULT 'UPCOMING',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(school_id, name)
);

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

CREATE TABLE academic_calendar (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    date DATE NOT NULL,
    type VARCHAR(50) NOT NULL,
    title VARCHAR(255) NOT NULL,
    description TEXT,
    is_working_day BOOLEAN DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE classes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    code VARCHAR(50) NOT NULL,
    sequence INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(school_id, code)
);

CREATE TABLE sections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    code VARCHAR(50) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(school_id, code)
);

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

CREATE TABLE class_sections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    academic_year_id UUID NOT NULL REFERENCES academic_years(id) ON DELETE CASCADE,
    class_id UUID NOT NULL REFERENCES classes(id) ON DELETE CASCADE,
    section_id UUID NOT NULL REFERENCES sections(id) ON DELETE CASCADE,
    class_teacher_staff_id UUID, 
    capacity INT,
    room_id UUID REFERENCES rooms(id),
    status VARCHAR(50) DEFAULT 'ACTIVE',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(academic_year_id, class_id, section_id)
);

CREATE TABLE subjects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    name VARCHAR(100) NOT NULL,
    code VARCHAR(50) NOT NULL,
    subject_type VARCHAR(50),
    description TEXT,
    status VARCHAR(50) DEFAULT 'ACTIVE',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(school_id, code)
);

-- ==========================================
-- 5. TRIGGERS & UTILITIES
-- ==========================================
CREATE OR REPLACE FUNCTION trigger_set_timestamp()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER set_timestamp_schools BEFORE UPDATE ON schools FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
CREATE TRIGGER set_timestamp_users BEFORE UPDATE ON users FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
CREATE TRIGGER set_timestamp_roles BEFORE UPDATE ON roles FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
CREATE TRIGGER set_timestamp_academic_years BEFORE UPDATE ON academic_years FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
CREATE TRIGGER set_timestamp_academic_terms BEFORE UPDATE ON academic_terms FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
CREATE TRIGGER set_timestamp_classes BEFORE UPDATE ON classes FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
CREATE TRIGGER set_timestamp_sections BEFORE UPDATE ON sections FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
CREATE TRIGGER set_timestamp_rooms BEFORE UPDATE ON rooms FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
CREATE TRIGGER set_timestamp_class_sections BEFORE UPDATE ON class_sections FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();
CREATE TRIGGER set_timestamp_subjects BEFORE UPDATE ON subjects FOR EACH ROW EXECUTE PROCEDURE trigger_set_timestamp();


-- Add System-Level Role Enum
CREATE TYPE system_role_type AS ENUM (
    'SUPER_ADMIN',  -- Eravaya platform administrators
    'SCHOOL_ADMIN', -- Principal / School Owners
    'STAFF',        -- General staff (HR, Accounts, etc)
    'TEACHER',      -- Academic teachers
    'STUDENT',      -- Enrolled students
    'PARENT'        -- Guardians
);

ALTER TABLE users ADD COLUMN user_type system_role_type NOT NULL DEFAULT 'STUDENT';

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
