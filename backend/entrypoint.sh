#!/bin/bash
set -e

echo "🚀 Starting Eravaya Backend Container..."

# 1. Run test suite with failure tracking and alert dispatch if cargo toolchain exists
if [ "${RUN_TESTS_ON_STARTUP:-false}" = "true" ] && command -v cargo &> /dev/null; then
    echo "🔍 Executing automated pre-flight tests..."
    cargo run --bin test_runner || {
        TEST_STATUS=$?
        echo "❌ Pre-flight tests failed! Alert email dispatched to admin."
        if [ "${STRICT_TESTS:-true}" = "true" ]; then
            echo "🛑 Stopping container startup due to test failure."
            exit $TEST_STATUS
        fi
    }
fi

# 2. Start the main Rust HTTP server
echo "✨ Launching Eravaya service..."
exec /app/eravaya
