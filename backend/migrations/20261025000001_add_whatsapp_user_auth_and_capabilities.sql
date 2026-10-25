-- Migration: Add WhatsApp User Sessions, Capabilities, and Verification Tokens

-- 1. WhatsApp User Sessions (tracks authenticated WhatsApp users and sliding expiration)
CREATE TABLE IF NOT EXISTS whatsapp_user_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    sender_jid TEXT NOT NULL UNIQUE,
    phone VARCHAR(50) NOT NULL,
    role_code VARCHAR(100) NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_whatsapp_user_sessions_jid
    ON whatsapp_user_sessions(sender_jid);

CREATE INDEX IF NOT EXISTS idx_whatsapp_user_sessions_user
    ON whatsapp_user_sessions(user_id);

CREATE INDEX IF NOT EXISTS idx_whatsapp_user_sessions_phone
    ON whatsapp_user_sessions(phone);

-- 2. WhatsApp Capabilities (caches authorized capabilities for the active WhatsApp session)
CREATE TABLE IF NOT EXISTS whatsapp_capabilities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id UUID NOT NULL REFERENCES whatsapp_user_sessions(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role_code VARCHAR(100) NOT NULL,
    capability_key VARCHAR(100) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (session_id, capability_key)
);

CREATE INDEX IF NOT EXISTS idx_whatsapp_capabilities_session
    ON whatsapp_capabilities(session_id);

CREATE INDEX IF NOT EXISTS idx_whatsapp_capabilities_user
    ON whatsapp_capabilities(user_id);

-- 3. WhatsApp Web Verification Tokens (used for web onboarding & OTP authentication)
CREATE TABLE IF NOT EXISTS whatsapp_verification_tokens (
    token VARCHAR(64) PRIMARY KEY,
    sender_jid TEXT NOT NULL,
    phone VARCHAR(50) NOT NULL,
    otp_code VARCHAR(10),
    otp_expires_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ NOT NULL,
    verified BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_whatsapp_verification_tokens_phone
    ON whatsapp_verification_tokens(phone);
