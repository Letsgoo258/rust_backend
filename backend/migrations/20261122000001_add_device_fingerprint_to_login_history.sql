-- ERAVAYA ERP - Add device fingerprint and performance indexes to login_history
ALTER TABLE login_history ADD COLUMN IF NOT EXISTS device_fingerprint TEXT;

CREATE INDEX IF NOT EXISTS idx_login_history_status ON login_history(status);
CREATE INDEX IF NOT EXISTS idx_login_history_ip_address ON login_history(ip_address);
CREATE INDEX IF NOT EXISTS idx_login_history_device_fingerprint ON login_history(device_fingerprint);
