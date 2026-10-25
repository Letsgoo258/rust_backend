-- Migration: 20261025000003_add_school_settings_and_admission_otps.sql
-- Supports school-specific configuration (SRD Section 6.1.3) and admission applicant OTP verification

-- 1. Create school_settings table
CREATE TABLE IF NOT EXISTS school_settings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    key VARCHAR(100) NOT NULL,
    value JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(school_id, key)
);

CREATE INDEX IF NOT EXISTS idx_school_settings_school_key ON school_settings(school_id, key);

-- 2. Create admission_otps table for phone & email verification
CREATE TABLE IF NOT EXISTS admission_otps (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    target VARCHAR(255) NOT NULL,
    target_type VARCHAR(20) NOT NULL, -- 'PHONE' or 'EMAIL'
    otp_code VARCHAR(10) NOT NULL,
    verified BOOLEAN NOT NULL DEFAULT false,
    verification_token VARCHAR(64),
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_admission_otps_target ON admission_otps(target, target_type);
CREATE INDEX IF NOT EXISTS idx_admission_otps_token ON admission_otps(verification_token);
