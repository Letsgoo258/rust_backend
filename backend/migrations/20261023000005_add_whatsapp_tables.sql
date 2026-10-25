-- PostgreSQL Schema for WhatsApp Service

-- Outbound message queue
CREATE TABLE IF NOT EXISTS whatsapp_outbox (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    recipient_jid TEXT NOT NULL,                         -- e.g. '919876543210@s.whatsapp.net'
    message_type TEXT NOT NULL DEFAULT 'text',           -- 'text', 'image', 'document', etc.
    payload JSONB NOT NULL,                              -- { "text": "..." } or media reference
    status TEXT NOT NULL DEFAULT 'pending',              -- 'pending', 'processing', 'sent', 'failed', 'cancelled'
    priority INT NOT NULL DEFAULT 100,                   -- lower = earlier
    idempotency_key TEXT UNIQUE,                         -- optional, prevents duplicate sends
    scheduled_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    attempt_count INT NOT NULL DEFAULT 0,
    max_attempts INT NOT NULL DEFAULT 3,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_outbox_pending ON whatsapp_outbox (status, priority, scheduled_at)
WHERE status IN ('pending', 'processing');

-- Inbound message log (audit trail)
CREATE TABLE IF NOT EXISTS whatsapp_inbound (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    sender_jid TEXT NOT NULL,
    message_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    whatsapp_timestamp TIMESTAMPTZ NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_whatsapp_inbound_sender ON whatsapp_inbound (sender_jid, received_at DESC);

-- Rate limit state (persisted so restarts don't reset cooldowns)
CREATE TABLE IF NOT EXISTS whatsapp_rate_limit (
    key TEXT PRIMARY KEY,                                -- e.g. 'global', 'jid:9198...'
    window_start TIMESTAMPTZ NOT NULL,
    count INT NOT NULL DEFAULT 0,
    daily_count INT NOT NULL DEFAULT 0,
    daily_reset_at TIMESTAMPTZ NOT NULL
);

-- Session storage fallback & encrypted snapshot backup
CREATE TABLE IF NOT EXISTS whatsapp_session_store (
    key TEXT PRIMARY KEY,
    value BYTEA NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
