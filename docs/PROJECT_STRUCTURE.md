# Project Structure

```
src/
├── main.rs            # Application entry point, setup tracing, start server
├── app.rs             # Axum router construction, global state
├── config.rs          # Environment variable loading
├── state.rs           # Shared application state (Db pool, configs)
├── error.rs           # Global application error types
├── db.rs              # Database connection initialization
│
├── middleware/        # Axum/Tower middleware
│   ├── mod.rs
│   ├── auth.rs        # Authentication middleware wrapping axum-login
│   ├── authorization.rs # RBAC permissions
│   ├── tenant.rs      # Extracts and enforces school_id
│   └── tracing.rs     # Request tracing
│
├── routes/            # HTTP Controllers
│   ├── mod.rs
│   ├── auth.rs        # Login, logout
│   ├── academics.rs
│   ├── students.rs
│   ├── attendance.rs
│   ├── whatsapp.rs    # Webhook handler
│   └── health.rs      # Liveness/Readiness
│
├── modules/           # Core Domain Logic
│   ├── schools/       # Tenancy domain
│   ├── users/         # IAM domain
│   ├── academics/     # Terms, Classes, Subjects
│   ├── students/      # Enrollments, Guardians
│   └── attendance/    # Sessions, Records
│
└── services/          # Cross-cutting Domain Services
    ├── audit.rs       # Central audit logging
    ├── whatsapp.rs    # Outbound WhatsApp messages
    └── ai.rs          # NVIDIA API integration
```
