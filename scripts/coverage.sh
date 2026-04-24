#!/usr/bin/env bash
# Coverage report: unit tests (precise) + E2E subprocess (via instrumented binary).
#
# Usage:
#   ./scripts/coverage.sh                     # unit only (fast, offline)
#   IRIS_HOST=localhost ./scripts/coverage.sh # unit + E2E (requires live IRIS)
#
# Output: target/coverage/summary.txt
#
# Prerequisites:
#   ~/.cargo/bin/rustup component add llvm-tools
#   cargo install cargo-llvm-cov

set -euo pipefail

TOOLCHAIN_BIN=$(find ~/.rustup/toolchains -maxdepth 6 -name llvm-cov \
  -path "*/aarch64-apple-darwin/bin/*" 2>/dev/null | head -1 || \
  find ~/.rustup/toolchains -maxdepth 6 -name llvm-cov -path "*/bin/*" 2>/dev/null | head -1)
export LLVM_COV=${LLVM_COV:-$TOOLCHAIN_BIN}
export LLVM_PROFDATA=${LLVM_PROFDATA:-${TOOLCHAIN_BIN/llvm-cov/llvm-profdata}}

[[ -f "$LLVM_COV" ]] || { echo "ERROR: llvm-cov not found. Run: ~/.cargo/bin/rustup component add llvm-tools"; exit 1; }

PROFILE_DIR="$(pwd)/target/coverage/profiles"
SUMMARY="$(pwd)/target/coverage/summary.txt"
mkdir -p "$PROFILE_DIR"
rm -f "$PROFILE_DIR"/*.profraw "$PROFILE_DIR"/*.profdata "$PROFILE_DIR"/*.lcov

UNIT_TESTS=(
  test_connection_fixes test_doc_params test_elicitation_sweep
  test_scm_escaping test_tools_fixes test_workspace_config
  test_compile_params test_elicitation interop_unit_tests
  manifest_tests skills_tests vscode_config_tests
  mcp_handshake test_retry
  test_scm_unit test_search_unit test_skills_unit
  test_info_unit test_generate_unit test_discovery_unit
)

echo "=== Step 1: Unit tests (lib + integration, no IRIS needed) ==="
TEST_ARGS=(--lib)
for t in "${UNIT_TESTS[@]}"; do TEST_ARGS+=(--test "$t"); done

~/.cargo/bin/cargo-llvm-cov llvm-cov \
  --package iris-dev-core \
  "${TEST_ARGS[@]}" \
  --summary-only 2>&1 | grep -E "^[a-z/].*\.rs|^TOTAL"

echo ""
echo "=== Step 2: Build instrumented iris-dev binary ==="
RUSTFLAGS="-C instrument-coverage" cargo build -p iris-dev 2>&1 | tail -2
INSTRUMENTED="$(pwd)/target/debug/iris-dev"
echo "Instrumented binary: $INSTRUMENTED"

echo ""
echo "=== Step 3: E2E coverage via instrumented subprocess ==="
if [[ -n "${IRIS_HOST:-}" ]]; then
  COV_DIR="$PROFILE_DIR/e2e"
  mkdir -p "$COV_DIR"

  # Run each E2E test individually so we can collect per-test profraw
  # Pass the instrumented binary path via IRIS_DEV_BIN
  LLVM_PROFILE_FILE="$COV_DIR/iris-dev-%p.profraw" \
    IRIS_DEV_BIN="$INSTRUMENTED" \
    IRIS_HOST="${IRIS_HOST}" IRIS_WEB_PORT="${IRIS_WEB_PORT:-52780}" \
    IRIS_CONTAINER="${IRIS_CONTAINER:-iris-dev-iris}" \
    IRIS_USERNAME="${IRIS_USERNAME:-_SYSTEM}" \
    IRIS_PASSWORD="${IRIS_PASSWORD:-SYS}" IRIS_NAMESPACE="${IRIS_NAMESPACE:-USER}" \
    cargo test --test test_e2e -- --test-threads=1 2>&1 | tail -3

  NPROF=$(ls "$COV_DIR"/*.profraw 2>/dev/null | wc -l | tr -d ' ')
  echo "Collected $NPROF E2E profraw files from subprocess"

  if [[ "$NPROF" -gt 0 ]]; then
    "$LLVM_PROFDATA" merge -sparse "$COV_DIR"/*.profraw \
      -o "$COV_DIR/e2e-merged.profdata"
    echo ""
    echo "=== E2E subprocess coverage (iris-dev binary paths) ==="
    "$LLVM_COV" report \
      --instr-profile="$COV_DIR/e2e-merged.profdata" \
      --object="$INSTRUMENTED" \
      --ignore-filename-regex="(registry|cargo|rustc|test)" \
      2>/dev/null | grep -E "^[a-z/].*\.rs|^TOTAL" | tee "$PROFILE_DIR/e2e-summary.txt"
  fi
else
  echo "IRIS_HOST not set — skipping E2E subprocess coverage"
fi

echo ""
echo "=== Final: Unit coverage summary ==="
~/.cargo/bin/cargo-llvm-cov llvm-cov \
  --package iris-dev-core \
  "${TEST_ARGS[@]}" \
  --summary-only 2>&1 | grep "^TOTAL" | tee "$SUMMARY"

echo ""
echo "Reports:"
echo "  Unit summary:     $SUMMARY"
[[ -f "$PROFILE_DIR/e2e-summary.txt" ]] && echo "  E2E summary:      $PROFILE_DIR/e2e-summary.txt"
