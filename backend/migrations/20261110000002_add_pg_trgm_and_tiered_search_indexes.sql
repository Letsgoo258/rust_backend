-- Migration: 20261110000002_add_pg_trgm_and_tiered_search_indexes.sql
-- Enables pg_trgm for high performance fuzzy, prefix, and typo-tolerant tiered search

CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- GIN trigram indexes for sub-millisecond Level 1, 2, and 3 user search
CREATE INDEX IF NOT EXISTS idx_users_first_name_trgm
  ON users USING gin (first_name gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_users_last_name_trgm
  ON users USING gin (last_name gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_users_admission_number_trgm
  ON users USING gin (admission_number gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_users_username_trgm
  ON users USING gin (username gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_users_email_trgm
  ON users USING gin (email gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_users_phone_trgm
  ON users USING gin (phone gin_trgm_ops);
