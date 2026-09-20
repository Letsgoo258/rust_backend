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
