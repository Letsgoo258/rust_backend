-- Migration: Add WhatsApp Link Tokens and Anti-Spoofing Columns

-- 1. WhatsApp Link Tokens (for ERP-authenticated users generating one-time link codes)
CREATE TABLE IF NOT EXISTS whatsapp_link_tokens (
    token VARCHAR(32) PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    role_code VARCHAR(50) NOT NULL,
    ip_address VARCHAR(45) NOT NULL,
    user_agent TEXT,
    device_info JSONB DEFAULT '{}',
    consumed BOOLEAN NOT NULL DEFAULT false,
    consumed_by_jid TEXT,
    consumed_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_whatsapp_link_tokens_user
    ON whatsapp_link_tokens(user_id);

CREATE INDEX IF NOT EXISTS idx_whatsapp_link_tokens_expires
    ON whatsapp_link_tokens(expires_at);

-- 2. Anti-Spoofing and Activity Tracking columns on whatsapp_user_sessions
ALTER TABLE whatsapp_user_sessions
    ADD COLUMN IF NOT EXISTS ip_address VARCHAR(45),
    ADD COLUMN IF NOT EXISTS user_agent TEXT,
    ADD COLUMN IF NOT EXISTS device_fingerprint TEXT,
    ADD COLUMN IF NOT EXISTS last_active_at TIMESTAMPTZ DEFAULT NOW(),
    ADD COLUMN IF NOT EXISTS is_revoked BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS suspicious_reason TEXT,
    ADD COLUMN IF NOT EXISTS login_token VARCHAR(32);

CREATE INDEX IF NOT EXISTS idx_whatsapp_user_sessions_revoked
    ON whatsapp_user_sessions(is_revoked);
