# ERAVAYA School ERP - Technical Architecture

## 1. Product Overview
ERAVAYA is a multi-school SaaS platform. It provides a WhatsApp interface for operational, routine tasks, and a React Web interface for complex administrative workflows. Both interfaces connect to the same central Rust backend API.

## 2. Architecture
```text
                         ┌─────────────────────┐
                         │      React Web      │
                         │       Vercel        │
                         └──────────┬──────────┘
                                    │ HTTPS
                                    ▼
                         ┌─────────────────────┐
                         │      Rust API       │
                         │       Axum          │
                         │                     │
                         │ Controllers/Handlers│
                         │ Middleware          │
                         │ Services            │
                         │ Repositories        │
                         └───────┬───────┬─────┘
                                 │       │
                              SQLx      External APIs
                                 │       │
                                 ▼       ├── WhatsApp
                         ┌────────────┐   ├── AI
                         │ PostgreSQL │   ├── Email
                         │   Source   │   └── Storage
                         │  of Truth  │
                         └────────────┘
```

## 3. Technology Stack
- **Backend:** Rust, Tokio, Axum
- **Database:** PostgreSQL, SQLx
- **Infrastructure:** Docker, Render (MVP)
- **Object Storage:** Cloudflare R2
- **Interfaces:** WhatsApp Business API, React Frontend

## 4. Folder Structure
- `src/main.rs`: Entry point
- `src/middleware/`: Auth, tenancy, observability
- `src/routes/`: HTTP handlers grouped by domain
- `src/modules/`: Domain logic (schools, users, students, etc.)
- `src/services/`: Cross-cutting logic (audit, notifications)

## 5. Database Architecture
PostgreSQL is the single source of truth. The database is strictly multi-tenant using a `school_id` column.

## 6. Entity Relationships
- **School** -> **Academic Year** -> **Class/Section**
- **Student** -> **Enrollment** (tied to Academic Year and Class/Section)

## 7. Authentication
Managed via standard session-based or token-based authentication relying on established Rust crates (`axum-login`, `tower-sessions`, argon2).

## 8. Authorization
RBAC (Role-Based Access Control) enforced at the Rust middleware/service layer.

## 9. Multi-tenancy
All school-owned data requires `school_id` filtering on every query.

## 10. Middleware Architecture
A robust middleware stack handles Request IDs, Tenant Context, Authentication, Rate Limiting, and Tracing.

## 11. Controller/Handler Architecture
Handlers receive requests, extract context, and defer entirely to Services. They do not execute SQL.

## 12. Service Architecture
Services hold business rules, enforce permissions, execute transactions, and generate audit logs.

## 13. Repository Architecture
Repositories strictly handle PostgreSQL queries and persistence, abstracting SQLx from the services.

## 14. OOP/Rust Design Philosophy
Composition over inheritance. Traits for dependency injection where testability requires it.

## 15. API Conventions
RESTful JSON API with versioning (e.g., `/api/v1/...`). Standardized response format.

## 16. Error Handling
Centralized domain errors converted to safe HTTP status codes. Internal details are logged, never leaked.

## 17. Validation
Strict API boundary validation (DTOs) and database constraints.

## 18. Transactions
Atomic operations using SQLx PostgreSQL transactions for multi-step mutations.

## 19. Audit Logging
Business history is stored in an `audit_logs` table for traceability.

## 20. WhatsApp Architecture
Webhooks are verified, parsed for intent, and routed through the same ERP services as web requests.

## 21. AI Architecture
AI acts purely as an interpreter, outputting structured commands that pass through Rust's strict validation and RBAC.

## 22. Observability
OpenTelemetry, structured logging (`tracing`), and Sentry for errors.

## 23. Docker
Multi-stage build resulting in a minimal runtime image.

## 24. Render Deployment
Stateless containers configured via environment variables.

## 25. Environment Variables
`DATABASE_URL`, `APP_PORT`, `SESSION_SECRET`, etc.

## 26. Security Model
Secure by default. Parameterized SQL, robust RBAC, multi-tenant isolation, verified webhooks.

## 27. Testing Strategy
Unit tests for logic, integration tests for DB, explicitly testing tenant isolation and authorization rules.

## 28. Migration Strategy
Version-controlled SQL files managed by SQLx CLI.

## 29. Development Phases
1. Foundation -> 2. Security -> 3. Academic Foundation -> 4. Attendance -> 5. API -> 6. Integrations.

## 30. Future Expansion Strategy
Additional modules (Exams, Fees) will plug into the existing RBAC and Tenant architecture without altering the core.
