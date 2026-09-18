# Observability Documentation

## Stack
- `tracing` for structured logs.
- OpenTelemetry for distributed tracing.
- Sentry for crash reporting.

Logs must contain `trace_id`, `school_id`, `user_id`. Never log secrets.
