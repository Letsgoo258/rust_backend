-- 1. Add updated_by_user_id column to class_sections table
ALTER TABLE class_sections
ADD COLUMN IF NOT EXISTS updated_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL;

-- 2. Create class_section_history table for 30-day change tracking
CREATE TABLE IF NOT EXISTS class_section_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    school_id UUID NOT NULL REFERENCES schools(id) ON DELETE CASCADE,
    class_section_id UUID NOT NULL REFERENCES class_sections(id) ON DELETE CASCADE,
    updated_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    change_type VARCHAR(100) NOT NULL,
    old_value TEXT,
    new_value TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Index for fast retrieval by class_section_id & pruning by created_at
CREATE INDEX IF NOT EXISTS idx_class_section_history_cs_id ON class_section_history(class_section_id);
CREATE INDEX IF NOT EXISTS idx_class_section_history_created_at ON class_section_history(created_at);
