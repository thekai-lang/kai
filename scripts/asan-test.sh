#!/usr/bin/env bash
# ASan leak-detection test suite for Kai leak regression fixtures.
#
# Usage:
#   ./scripts/asan-test.sh              # build + test
#   ./scripts/asan-test.sh --no-build   # skip build, just run tests
#
# Exit: 0 if all pass, 1 if any fail.
# Requires: cargo +nightly, x86_64-unknown-linux-gnu target.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
FIXTURE_DIR="$ROOT_DIR/tests/fixtures/leak"
TARGET_DIR="$ROOT_DIR/target-asan"
BINARY="$TARGET_DIR/x86_64-unknown-linux-gnu/release/kai"

# expected exit code per fixture — verified via manual JIT run before commit.
declare -A EXPECTED=(
    ["minimal.kai"]=1
    ["minimal2.kai"]=5
    ["test3.kai"]=99
    ["test4.kai"]=99
    ["test5.kai"]=99
    ["stress_heap.kai"]=99
)

BUILD=1
if [[ "${1:-}" == "--no-build" ]]; then
    BUILD=0
fi

if [[ "$BUILD" -eq 1 ]]; then
    echo ">>> Building ASan binary..."
    CARGO_TARGET_DIR="$TARGET_DIR" \
    RUSTFLAGS="-Zsanitizer=address" \
        cargo +nightly build --release --target x86_64-unknown-linux-gnu \
        -p kai-driver 2>&1
    echo
fi

if [[ ! -x "$BINARY" ]]; then
    echo "FATAL: ASan binary not found at $BINARY"
    echo "Run without --no-build, or build manually first."
    exit 1
fi

PASS=0
FAIL=0
TOTAL=0

for fixture in "$FIXTURE_DIR"/*.kai; do
    name="$(basename "$fixture")"
    expected="${EXPECTED[$name]:-}"
    TOTAL=$((TOTAL + 1))

    if [[ -z "$expected" ]]; then
        echo "SKIP  $name (no expected exit code defined)"
        continue
    fi

    # Run under ASan, capture both stdout+stderr and exit code.
    exit_code=0
    output=$(ASAN_OPTIONS="detect_leaks=1:leak_check_at_exit=1" \
        "$BINARY" run "$fixture" 2>&1) || exit_code=$?

    # Check for LeakSanitizer errors first — these are the primary failure.
    if echo "$output" | grep -q "ERROR: LeakSanitizer"; then
        echo "FAIL  $name — LEAK DETECTED (exit $exit_code, expected $expected)"
        echo "$output" | grep -A2 "LeakSanitizer"
        echo
        FAIL=$((FAIL + 1))
        continue
    fi

    # Check for other ASan errors (use-after-free, double-free, etc.).
    if echo "$output" | grep -q "ERROR: AddressSanitizer"; then
        echo "FAIL  $name — ASAN ERROR (exit $exit_code, expected $expected)"
        echo "$output" | grep -A2 "AddressSanitizer"
        echo
        FAIL=$((FAIL + 1))
        continue
    fi

    # Check exit code matches expected.
    if [[ "$exit_code" -ne "$expected" ]]; then
        echo "FAIL  $name — wrong exit code: got $exit_code, expected $expected"
        [[ -n "$output" ]] && echo "$output"
        echo
        FAIL=$((FAIL + 1))
        continue
    fi

    echo "OK    $name (exit $exit_code)"
    PASS=$((PASS + 1))
done

echo
echo "=== Results: $PASS passed, $FAIL failed, $TOTAL total ==="
[[ "$FAIL" -eq 0 ]] && exit 0 || exit 1
