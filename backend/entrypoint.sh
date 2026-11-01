#!/bin/bash
set -e

echo "🚀 Starting Eravaya Backend Container..."

# 1. Run test suite with failure tracking and alert dispatch
if [ "${RUN_TESTS_ON_STARTUP:-false}" = "true" ]; then
    echo "🔍 Executing automated pre-flight tests..."
    TEST_CMD=""
    if [ -x /app/test_runner ]; then
        TEST_CMD="/app/test_runner"
    elif command -v cargo &> /dev/null; then
        TEST_CMD="cargo run --release --bin test_runner"
    fi

    if [ -n "$TEST_CMD" ]; then
        $TEST_CMD || {
            TEST_STATUS=$?
            echo "❌ Pre-flight tests failed! Alert email dispatched to admin."
            if [ "${STRICT_TESTS:-true}" = "true" ]; then
                echo "🛑 Stopping container startup due to test failure."
                exit $TEST_STATUS
            fi
        }
    fi
fi

# 2. Start the main Rust HTTP server
echo "✨ Launching Eravaya service..."
exec /app/eravaya
