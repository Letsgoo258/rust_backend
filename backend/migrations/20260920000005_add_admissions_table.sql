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
