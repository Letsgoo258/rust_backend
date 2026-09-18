# API Documentation

## Base URL
`/api/v1/`

## Standard Response Format
**Success:**
```json
{
  "success": true,
  "data": { ... }
}
```

**Error:**
```json
{
  "success": false,
  "error": {
    "code": "ERROR_CODE",
    "message": "User friendly message"
  },
  "trace_id": "xyz123"
}
```
