-- Run this script directly in your Aiven PostgreSQL console using the `avnadmin` user.
-- It creates a dedicated application user and a read-only analytics user.

-- 1. Create the backend application user (Least Privilege)
CREATE ROLE eravaya_app WITH LOGIN PASSWORD 'CHANGE_THIS_TO_A_SECURE_PASSWORD';
GRANT CONNECT ON DATABASE defaultdb TO eravaya_app;
GRANT USAGE ON SCHEMA public TO eravaya_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO eravaya_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO eravaya_app;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO eravaya_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT USAGE, SELECT ON SEQUENCES TO eravaya_app;

-- 2. Create a Read-Only user (For Grafana / Analytics / Metabase)
CREATE ROLE eravaya_readonly WITH LOGIN PASSWORD 'CHANGE_THIS_TO_A_SECURE_PASSWORD';
GRANT CONNECT ON DATABASE defaultdb TO eravaya_readonly;
GRANT USAGE ON SCHEMA public TO eravaya_readonly;
GRANT SELECT ON ALL TABLES IN SCHEMA public TO eravaya_readonly;
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT ON TABLES TO eravaya_readonly;
