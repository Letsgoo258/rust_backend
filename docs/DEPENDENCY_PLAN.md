# Dependency Plan

As per the non-negotiable technology stack and security-first policies:

## Core Dependencies
- `axum`: Web framework.
- `tokio`: Async runtime.
- `serde` & `serde_json`: Serialization.
- `sqlx`: Database driver (PostgreSQL).

## Security & Authentication
- `axum-login`: High-level session authentication.
- `tower-sessions`: Session management.
- `tower-sessions-sqlx-store`: PostgreSQL backend for sessions.
- `argon2`: Secure password hashing.

## Observability
- `tracing`: Structured logging.
- `tracing-subscriber`: Log emission.
- `tower-http`: HTTP request tracing and metrics.
- (Later): Sentry and OpenTelemetry crates.

## Utilities
- `uuid`: For generating and parsing standard identifiers.
- `chrono` or `time`: For datetime handling.
- `dotenvy`: Environment variable management.
- `thiserror`: Ergonomic error handling.
- `reqwest`: For WhatsApp API/AI webhooks.
