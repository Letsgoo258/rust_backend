-- Migration: 20261101000002_add_tc_templates_and_versions.sql
-- Supports reusable certificate template builder with versioning, snapshots, and audit support

CREATE TABLE IF NOT EXISTS tc_templates (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    template_type VARCHAR(50) NOT NULL DEFAULT 'transfer_certificate',
    name VARCHAR(255) NOT NULL DEFAULT 'Standard Transfer Certificate',
    config JSONB NOT NULL DEFAULT '{}'::jsonb,
    schema_version INT NOT NULL DEFAULT 1,
    is_published BOOLEAN NOT NULL DEFAULT false,
    version INT NOT NULL DEFAULT 1,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    updated_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(school_id, template_type)
);

CREATE INDEX IF NOT EXISTS idx_tc_templates_school_type ON tc_templates(school_id, template_type);

CREATE TABLE IF NOT EXISTS tc_template_versions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    template_id UUID NOT NULL REFERENCES tc_templates(id) ON DELETE CASCADE,
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    version INT NOT NULL,
    snapshot JSONB NOT NULL,
    diff_summary TEXT,
    created_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_tc_template_versions_tpl ON tc_template_versions(template_id, version DESC);
