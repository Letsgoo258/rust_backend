-- 1. Create universal entity_change_history table for 30-day change tracking
CREATE TABLE IF NOT EXISTS entity_change_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    entity_type VARCHAR(100) NOT NULL, -- e.g., 'USER', 'CLASS_SECTION', 'ACADEMIC_YEAR', 'SUBJECT', 'SCHOOL'
    entity_id UUID NOT NULL,
    action VARCHAR(50) NOT NULL, -- 'CREATE', 'UPDATE', 'DELETE'
    updated_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    before_state JSONB,
    after_state JSONB,
    ip_address VARCHAR(45),
    user_agent TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Indexes for fast audit retrieval and 30-day pruning
CREATE INDEX IF NOT EXISTS idx_entity_change_history_entity ON entity_change_history(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_entity_change_history_school ON entity_change_history(school_id);
CREATE INDEX IF NOT EXISTS idx_entity_change_history_created_at ON entity_change_history(created_at);

-- 2. Add last_history_id and updated_by_user_id references to essential tables
ALTER TABLE schools
ADD COLUMN IF NOT EXISTS last_history_id UUID REFERENCES entity_change_history(id) ON DELETE SET NULL,
ADD COLUMN IF NOT EXISTS updated_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE users
ADD COLUMN IF NOT EXISTS last_history_id UUID REFERENCES entity_change_history(id) ON DELETE SET NULL;

ALTER TABLE class_sections
ADD COLUMN IF NOT EXISTS last_history_id UUID REFERENCES entity_change_history(id) ON DELETE SET NULL;

ALTER TABLE academic_years
ADD COLUMN IF NOT EXISTS last_history_id UUID REFERENCES entity_change_history(id) ON DELETE SET NULL,
ADD COLUMN IF NOT EXISTS updated_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL;

ALTER TABLE subjects
ADD COLUMN IF NOT EXISTS last_history_id UUID REFERENCES entity_change_history(id) ON DELETE SET NULL,
ADD COLUMN IF NOT EXISTS updated_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL;
