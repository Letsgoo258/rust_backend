-- Migration: Remove legacy `students` table and consolidate student profile data into `users` table.

-- 1. Add student fields to `users` table
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS admission_number VARCHAR(50),
    ADD COLUMN IF NOT EXISTS date_of_birth DATE,
    ADD COLUMN IF NOT EXISTS gender VARCHAR(20),
    ADD COLUMN IF NOT EXISTS blood_group VARCHAR(10);

-- 2. Unique constraint on (school_id, admission_number) in users table
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'users_school_id_admission_number_key'
    ) THEN
        ALTER TABLE users ADD CONSTRAINT users_school_id_admission_number_key UNIQUE (school_id, admission_number);
    END IF;
END $$;

-- 3. Migrate data from existing `students` table into `users` table if `students` exists
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = 'students') THEN
        UPDATE users u
        SET
            admission_number = s.admission_number,
            date_of_birth = s.date_of_birth,
            gender = COALESCE(u.gender, s.gender),
            blood_group = COALESCE(u.blood_group, s.blood_group),
            updated_at = NOW()
        FROM students s
        WHERE s.user_id = u.id;

        -- 4. Repoint `enrollments` foreign key from `students(id)` to `users(id)`
        -- Check if student_id in enrollments references students(id) vs users(id)
        ALTER TABLE enrollments DROP CONSTRAINT IF EXISTS enrollments_student_id_fkey;

        -- Update enrollments.student_id from students.id to students.user_id if needed
        UPDATE enrollments e
        SET student_id = s.user_id
        FROM students s
        WHERE e.student_id = s.id AND s.user_id IS NOT NULL;

        ALTER TABLE enrollments
            ADD CONSTRAINT enrollments_student_id_fkey
            FOREIGN KEY (student_id) REFERENCES users(id) ON DELETE CASCADE ON UPDATE CASCADE;

        -- 5. Repoint `student_guardians` foreign key from `students(id)` to `users(id)`
        ALTER TABLE student_guardians DROP CONSTRAINT IF EXISTS student_guardians_student_id_fkey;

        UPDATE student_guardians sg
        SET student_id = s.user_id
        FROM students s
        WHERE sg.student_id = s.id AND s.user_id IS NOT NULL;

        ALTER TABLE student_guardians
            ADD CONSTRAINT student_guardians_student_id_fkey
            FOREIGN KEY (student_id) REFERENCES users(id) ON DELETE CASCADE ON UPDATE CASCADE;

        -- 6. Drop legacy indices and table `students`
        DROP INDEX IF EXISTS idx_students_upper_admission_number;
        DROP INDEX IF EXISTS idx_students_school_id;
        DROP TABLE IF EXISTS students CASCADE;
    END IF;
END $$;

-- 7. Functional index for fast upper-case admission_number lookups on `users` table
CREATE INDEX IF NOT EXISTS idx_users_upper_admission_number ON users (school_id, UPPER(admission_number));
