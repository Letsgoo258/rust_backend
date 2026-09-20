ALTER TABLE users ADD COLUMN imported_via_csv BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE users ADD COLUMN updated_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL;

-- Automatically clean up audit logs older than 30 days to save space and maintain compliance
-- We will create a small function to do this, though typically it's called by a cron job or application logic
