-- Migration: 20261109000002_prevent_duplicate_active_transfers.sql
-- Enforces that a student can only have ONE active/valid issued certificate per report type.
-- Cancels duplicate test records and resets sequence counters.

-- 1. For any duplicate ISSUED reports for the same student and report_type, keep the lowest serial_number and cancel the rest
WITH ranked_reports AS (
    SELECT id,
           ROW_NUMBER() OVER (
               PARTITION BY school_id, student_id, report_type
               ORDER BY serial_number ASC, created_at ASC
           ) as rn
    FROM issued_reports
    WHERE status = 'ISSUED' AND student_id IS NOT NULL
)
UPDATE issued_reports
SET status = 'CANCELLED',
    remarks = COALESCE(remarks, '') || ' [Cancelled redundant duplicate issuance]'
WHERE id IN (
    SELECT id FROM ranked_reports WHERE rn > 1
);

-- 2. Resynchronize sequence current_number with highest active serial_number per report_type
UPDATE report_sequences rs
SET current_number = COALESCE(
    (SELECT MAX(ir.serial_number) 
     FROM issued_reports ir 
     WHERE ir.school_id = rs.school_id 
       AND ir.report_type = rs.report_type 
       AND ir.status = 'ISSUED'), 
    0
)
WHERE EXISTS (
    SELECT 1 FROM issued_reports ir 
    WHERE ir.school_id = rs.school_id AND ir.report_type = rs.report_type
);

-- 3. Create partial unique index guaranteeing AT MOST ONE active (ISSUED) report per student per report_type
CREATE UNIQUE INDEX IF NOT EXISTS idx_one_active_report_per_student
ON issued_reports (school_id, student_id, report_type)
WHERE status = 'ISSUED';
