-- 20261023000003_add_role_capabilities_index.sql
-- Expression index for fast capability lookups by (school_id, UPPER(role_code))

CREATE INDEX IF NOT EXISTS idx_role_capabilities_school_upper_role
  ON role_capabilities (school_id, UPPER(role_code));
