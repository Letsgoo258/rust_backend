-- Migration: Add user demographic fields and custom field value storage
-- 1. Add caste_category (Social Category) and custom_values JSONB to users table
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS caste_category VARCHAR(100),
    ADD COLUMN IF NOT EXISTS custom_values JSONB DEFAULT '{}'::jsonb;

-- 2. Create index on caste_category for demographic filtering and TC generation
CREATE INDEX IF NOT EXISTS idx_users_caste_category ON users (school_id, caste_category);

-- 3. Create user_custom_values table for relational integrity with custom_fields table
CREATE TABLE IF NOT EXISTS user_custom_values (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    field_id UUID NOT NULL REFERENCES custom_fields(id) ON DELETE CASCADE,
    value TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(user_id, field_id)
);

CREATE INDEX IF NOT EXISTS idx_user_custom_values_user ON user_custom_values (user_id);
CREATE INDEX IF NOT EXISTS idx_user_custom_values_field ON user_custom_values (field_id);
