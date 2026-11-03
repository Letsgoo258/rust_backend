-- Migration: 20261101000004_populate_sequential_roll_numbers.sql
-- Assigns sequential class roll numbers (1..N) per section based on canonical roster ordering,
-- eliminating roll collisions and aligning attendance indices with enrollment records.

WITH ranked AS (
    SELECT 
        e.id as enrollment_id,
        ROW_NUMBER() OVER (
            PARTITION BY e.class_section_id 
            ORDER BY COALESCE(NULLIF(u.admission_number, ''), '999999'), 
                     COALESCE(NULLIF(u.display_name, ''), NULLIF(TRIM(CONCAT(u.first_name, ' ', u.last_name)), ''), u.username) ASC
        )::TEXT as seq_roll
    FROM enrollments e
    JOIN users u ON e.student_id = u.id
    WHERE e.class_section_id IS NOT NULL
)
UPDATE enrollments
SET roll_number = ranked.seq_roll,
    updated_at = NOW()
FROM ranked
WHERE enrollments.id = ranked.enrollment_id;

-- Fix incorrectly marked test student in session acc886ba-bf34-4b0b-90f7-e0ad2178c0e6
UPDATE attendance_records
SET status = 'PRESENT',
    remarks = NULL
WHERE attendance_session_id = 'acc886ba-bf34-4b0b-90f7-e0ad2178c0e6'
  AND student_id = (SELECT id FROM users WHERE admission_number = 'TEST-SNGL-576A0B' LIMIT 1);
