-- Speed up LMS Daily Report query on attendance_sessions filtering by school_id, date, and marked_by
CREATE INDEX IF NOT EXISTS idx_attendance_sessions_school_date_marked 
ON attendance_sessions(school_id, date, marked_by);
