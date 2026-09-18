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
