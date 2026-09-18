# Database Documentation

## Principles
- **Source of Truth:** PostgreSQL.
- **Tenancy:** Strict `school_id` scoping.
- **Types:** `UUID` for external IDs, `NUMERIC` for financial, `TIMESTAMPTZ` for precise times.
- **Integrity:** Heavy use of FKs, Check Constraints, and Unique Constraints.

See `TECHDOC_SCHEMA_AUDIT.md` for specific corrections made to the initial specification.
