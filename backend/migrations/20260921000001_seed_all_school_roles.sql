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
