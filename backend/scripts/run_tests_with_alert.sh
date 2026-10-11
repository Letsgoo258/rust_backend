#!/usr/bin/env bash
set -eo pipefail

ADMIN_EMAIL="${ALERT_ADMIN_EMAIL:-kabhiram67@gmail.com}"
TRACE_ID=$(uuidgen 2>/dev/null || cat /proc/sys/kernel/random/uuid 2>/dev/null || od -x /dev/urandom | head -1 | awk '{print $2$3$4$5}')
TIMESTAMP=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

echo "============================================================"
echo "🧪 Running Test Suite with Automated Alerting"
echo "🆔 Trace ID: ${TRACE_ID}"
echo "⏰ Timestamp: ${TIMESTAMP}"
echo "============================================================"

# Create temporary log capture
LOG_FILE=$(mktemp)

if cargo test -- --nocapture > "$LOG_FILE" 2>&1; then
    echo "✅ All tests passed successfully!"
    cat "$LOG_FILE"
    rm -f "$LOG_FILE"
    exit 0
else
    EXIT_CODE=$?
    echo "❌ Test suite dropped/failed with exit code: ${EXIT_CODE}"
    cat "$LOG_FILE"

    echo "🚨 Dispatching failure alert to ${ADMIN_EMAIL} with Trace ID ${TRACE_ID}..."

    # Try sending via compiled test_runner if available, or direct email
    if [ -f "/app/test_runner" ] || command -v cargo &> /dev/null; then
        cargo run --bin test_runner || true
    fi

    rm -f "$LOG_FILE"
    exit "${EXIT_CODE}"
fi
