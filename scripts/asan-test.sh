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
LEAK_DIR="$ROOT_DIR/tests/fixtures/leak"
REV_DIR="$ROOT_DIR/tests/fixtures/reversible"
TARGET_DIR="$ROOT_DIR/target-asan"
BINARY="$TARGET_DIR/x86_64-unknown-linux-gnu/release/kai"

# expected exit code per leak fixture — verified via manual JIT run before commit.
declare -A EXPECTED=(
    ["minimal.kai"]=1
    ["minimal2.kai"]=5
    ["test3.kai"]=99
    ["test4.kai"]=99
    ["test5.kai"]=99
    ["stress_heap.kai"]=99
)

# Reversible fixtures (§5.3). Commit-path fixtures exit with their exact value
# (must be leak-FREE — every retained OLD snapshot claim is released on commit);
# unwind-path fixtures exit 101 (terminal §10.1 panic) and must NOT abort on a
# refcount underflow during rollback.
declare -A REV_EXPECTED=(
    ["scalar_commit.kai"]=7
    ["heap_commit.kai"]=31
    ["lifo_commit.kai"]=63
    ["nested_commit.kai"]=7
    ["stress_loop.kai"]=127
    ["stress_deep.kai"]=255
    ["unwind_basic.kai"]=101
    ["unwind_lifo.kai"]=101
    ["nested_unwind.kai"]=101
    ["unwind_deep.kai"]=101
)

# Fixtures whose expected exit is 101 (terminal panic) additionally require the
# unwind to complete WITHOUT a refcount-underflow abort.
REV_UNDERFLOW_CHECKS=(unwind_basic.kai unwind_lifo.kai nested_unwind.kai unwind_deep.kai)

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

# Run one fixture under ASan against an expected exit code (+ optional
# no-underflow check for terminal-panic fixtures). Ok/Fail counters are global.
check_fixture() {
    local fixture="$1" expected="$2" check_underflow="${3:-0}"
    local name exit_code output

    name="$(basename "$fixture")"
    TOTAL=$((TOTAL + 1))

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
        return
    fi

    # Check for other ASan errors (use-after-free, double-free, etc.).
    if echo "$output" | grep -q "ERROR: AddressSanitizer"; then
        echo "FAIL  $name — ASAN ERROR (exit $exit_code, expected $expected)"
        echo "$output" | grep -A2 "AddressSanitizer"
        echo
        FAIL=$((FAIL + 1))
        return
    fi

    # Reversible unwind must NOT abort on a refcount underflow during rollback.
    if [[ "$check_underflow" -eq 1 ]] && echo "$output" | grep -q "refcount underflow"; then
        echo "FAIL  $name — REFCOUNT UNDERFLOW during unwind"
        echo
        FAIL=$((FAIL + 1))
        return
    fi

    # Check exit code matches expected.
    if [[ "$exit_code" -ne "$expected" ]]; then
        echo "FAIL  $name — wrong exit code: got $exit_code, expected $expected"
        [[ -n "$output" ]] && echo "$output"
        echo
        FAIL=$((FAIL + 1))
        return
    fi

    echo "OK    $name (exit $exit_code)"
    PASS=$((PASS + 1))
}

echo "== leak regression fixtures =="
for fixture in "$LEAK_DIR"/*.kai; do
    expected="${EXPECTED[$(basename "$fixture")]:-}"
    [[ -z "$expected" ]] && { echo "SKIP  $(basename "$fixture") (no expected exit code defined)"; continue; }
    check_fixture "$fixture" "$expected" 0
done

echo
echo "== reversible fixtures (§5.3) =="
for fixture in "$REV_DIR"/*.kai; do
    name="$(basename "$fixture")"
    expected="${REV_EXPECTED[$name]:-}"
    [[ -z "$expected" ]] && { echo "SKIP  $name (no expected exit code defined)"; continue; }
    underflow=0
    for uf in "${REV_UNDERFLOW_CHECKS[@]}"; do
        [[ "$uf" == "$name" ]] && underflow=1
    done
    check_fixture "$fixture" "$expected" "$underflow"
done

echo
echo "=== Results: $PASS passed, $FAIL failed, $TOTAL total ==="
[[ "$FAIL" -eq 0 ]] && exit 0 || exit 1
