-- Default timezone for schools table to Asia/Kolkata
ALTER TABLE schools ALTER COLUMN timezone SET DEFAULT 'Asia/Kolkata';

UPDATE schools SET timezone = 'Asia/Kolkata' WHERE timezone IS NULL OR timezone = 'UTC';
