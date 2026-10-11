-- 1. Ensure all existing users.roles values are fully synchronized into user_roles join table
INSERT INTO user_roles (user_id, role_id)
SELECT DISTINCT u.id AS user_id, r.id AS role_id
FROM users u
CROSS JOIN LATERAL unnest(u.roles) AS r_code
JOIN roles r ON r.school_id = u.school_id AND (r.code = r_code::TEXT OR UPPER(r.code) = UPPER(r_code::TEXT))
ON CONFLICT (user_id, role_id) DO NOTHING;

-- 2. Drop the redundant roles array column on users table
ALTER TABLE users DROP COLUMN IF EXISTS roles;
