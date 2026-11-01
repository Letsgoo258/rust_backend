-- Migration: Add WhatsApp Conversation State table for multi-step interactive workflows (e.g. attendance)

CREATE TABLE IF NOT EXISTS whatsapp_conversation_state (
    session_id UUID PRIMARY KEY REFERENCES whatsapp_user_sessions(id) ON DELETE CASCADE,
    state_type VARCHAR(50) NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_whatsapp_conv_state_updated
    ON whatsapp_conversation_state(updated_at);
