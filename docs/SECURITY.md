# Security Documentation

## Rules
- **No Plaintext Passwords:** Use Argon2.
- **Tenant Isolation:** Every query must include `school_id` (unless globally scoped).
- **RBAC:** Enforce permissions in Rust middleware/services.
- **Webhooks:** Verify signatures and ensure idempotency.
