-- ERAVAYA ERP - Cascade Delete Cleanup & Constraint Fixes
-- Ensures all user profiles (staff, students, guardians) and school associations cascade cleanly on deletion.

-- 1. Clean up orphaned staff, students, and guardians rows that have NULL user_id or invalid school_id
DELETE FROM staff WHERE user_id IS NULL OR school_id NOT IN (SELECT id FROM schools);
DELETE FROM students WHERE user_id IS NULL OR school_id NOT IN (SELECT id FROM schools);
DELETE FROM guardians WHERE user_id IS NULL OR school_id NOT IN (SELECT id FROM schools);

-- 2. Update staff user_id foreign key constraint to CASCADE on delete
ALTER TABLE staff
    DROP CONSTRAINT IF EXISTS staff_user_id_fkey,
    ADD CONSTRAINT staff_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE ON UPDATE CASCADE;

-- 3. Update students user_id foreign key constraint to CASCADE on delete
ALTER TABLE students
    DROP CONSTRAINT IF EXISTS students_user_id_fkey,
    ADD CONSTRAINT students_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE ON UPDATE CASCADE;

-- 4. Update guardians user_id foreign key constraint to CASCADE on delete
ALTER TABLE guardians
    DROP CONSTRAINT IF EXISTS guardians_user_id_fkey,
    ADD CONSTRAINT guardians_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE ON UPDATE CASCADE;

-- 5. Fix class_sections, rooms, and users branch foreign keys
ALTER TABLE class_sections
    DROP CONSTRAINT IF EXISTS class_sections_room_id_fkey,
    ADD CONSTRAINT class_sections_room_id_fkey
    FOREIGN KEY (room_id) REFERENCES rooms(id) ON DELETE SET NULL ON UPDATE CASCADE;

ALTER TABLE rooms
    DROP CONSTRAINT IF EXISTS rooms_branch_id_fkey,
    ADD CONSTRAINT rooms_branch_id_fkey
    FOREIGN KEY (branch_id) REFERENCES school_branches(id) ON DELETE CASCADE ON UPDATE CASCADE;

ALTER TABLE users
    DROP CONSTRAINT IF EXISTS users_branch_id_fkey,
    ADD CONSTRAINT users_branch_id_fkey
    FOREIGN KEY (branch_id) REFERENCES school_branches(id) ON DELETE SET NULL ON UPDATE CASCADE;
