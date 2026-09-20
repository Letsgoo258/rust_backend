-- ERAVAYA ERP - School Operable Flag & Onboarding Step Tracker

ALTER TABLE schools 
  ADD COLUMN IF NOT EXISTS operable BOOLEAN DEFAULT false,
  ADD COLUMN IF NOT EXISTS onboarding_step INT DEFAULT 1;

CREATE INDEX IF NOT EXISTS idx_schools_operable ON schools(operable);
