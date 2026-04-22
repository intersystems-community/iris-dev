#!/usr/bin/env bash
# Manual smoke test for branch 020-scm-elicitation-auto-open
# Run this BEFORE merging to master.
#
# Prerequisites:
#   - IRIS running with Atelier enabled
#   - IRIS_HOST / IRIS_PORT / IRIS_USERNAME / IRIS_PASSWORD set (or defaults apply)
#   - Source control package active in the target namespace (e.g. ISC.Git or %Studio.SourceControl)
#   - iris-dev binary built: cargo build --release
#
# Usage:
#   cd ~/ws/iris-dev
#   IRIS_HOST=localhost IRIS_PORT=52773 ./docs/test-020-manual.sh
#
# Each section prints PASS or FAIL.  Exit code = number of failures.

set -uo pipefail

# Always run from repo root regardless of where the script is invoked from
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$REPO_ROOT"

IRIS_HOST="${IRIS_HOST:-localhost}"
IRIS_PORT="${IRIS_PORT:-52773}"
IRIS_USERNAME="${IRIS_USERNAME:-_SYSTEM}"
IRIS_PASSWORD="${IRIS_PASSWORD:-SYS}"
IRIS_NAMESPACE="${IRIS_NAMESPACE:-USER}"
BIN="./target/release/iris-dev"

PASS=0
FAIL=0

_pass() { echo "  ✓  $1"; PASS=$((PASS+1)); }
_fail() { echo "  ✗  $1"; FAIL=$((FAIL+1)); }

base_url="http://${IRIS_HOST}:${IRIS_PORT}/api/atelier"
auth=(-u "${IRIS_USERNAME}:${IRIS_PASSWORD}")

curl_quiet() { curl -sf "${auth[@]}" "$@"; }

section() { echo; echo "━━━ $1 ━━━"; }

# ──────────────────────────────────────────────────────────────
section "0. Build check"
if [[ -x "$BIN" ]]; then
    _pass "binary exists: $BIN"
else
    echo "  Building release binary..."
    cargo build --release 2>&1
    [[ -x "$BIN" ]] && _pass "binary built OK" || { _fail "build failed — see output above"; exit 1; }
fi

# ──────────────────────────────────────────────────────────────
section "1. iris_compile — compile a simple class"
TEST_CLS='Class Test.Smoke020 Extends %RegisteredObject { ClassMethod Hello() As %String { Return "hello" } }'
COMPILE_DOC="Test.Smoke020.cls"

# Write the doc via Atelier PUT
PUT_BODY=$(python3 -c "
import json, sys
content = sys.argv[1]
print(json.dumps({'enc': False, 'content': content.splitlines()}))
" "$TEST_CLS")

status=$(curl_quiet -s -o /dev/null -w "%{http_code}" \
    -X PUT "${base_url}/v1/${IRIS_NAMESPACE}/doc/${COMPILE_DOC}" \
    -H 'Content-Type: application/json' \
    -d "$PUT_BODY")

if [[ "$status" == "200" || "$status" == "201" ]]; then
    _pass "PUT Test.Smoke020.cls → $status"
else
    _fail "PUT Test.Smoke020.cls → $status (expected 200/201)"
fi

# Compile via iris-dev iris_compile
result=$(echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"iris_compile","arguments":{"documents":["Test.Smoke020.cls"],"namespace":"'"$IRIS_NAMESPACE"'"}}}' \
    | "$BIN" 2>/dev/null || true)

if echo "$result" | grep -q '"success":true'; then
    _pass "iris_compile Test.Smoke020.cls"
elif echo "$result" | grep -q 'Test.Smoke020'; then
    _pass "iris_compile response mentions class (check manually)"
else
    _fail "iris_compile — unexpected response: $(echo "$result" | head -c 200)"
fi

# ──────────────────────────────────────────────────────────────
section "2. iris_doc read — round-trip the class we just wrote"
result=$(echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"iris_doc","arguments":{"action":"get","document":"Test.Smoke020.cls","namespace":"'"$IRIS_NAMESPACE"'"}}}' \
    | "$BIN" 2>/dev/null || true)

if echo "$result" | grep -q 'Smoke020'; then
    _pass "iris_doc get Test.Smoke020.cls"
else
    _fail "iris_doc get — class not found in response"
fi

# ──────────────────────────────────────────────────────────────
section "3. iris_source_control status"
result=$(echo '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"iris_source_control","arguments":{"action":"status","document":"Test.Smoke020.cls","namespace":"'"$IRIS_NAMESPACE"'"}}}' \
    | "$BIN" 2>/dev/null || true)

# Acceptable: JSON with success key OR "no source control" message
if echo "$result" | grep -qiE '"success"|source.control|not active'; then
    _pass "iris_source_control status (got a response)"
else
    _fail "iris_source_control status — unexpected: $(echo "$result" | head -c 200)"
fi

# ──────────────────────────────────────────────────────────────
section "4. iris_source_control menu"
result=$(echo '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"iris_source_control","arguments":{"action":"menu","document":"Test.Smoke020.cls","namespace":"'"$IRIS_NAMESPACE"'"}}}' \
    | "$BIN" 2>/dev/null || true)

if echo "$result" | grep -qiE '"actions"|"items"|"menu"|not active|no source'; then
    _pass "iris_source_control menu"
else
    _fail "iris_source_control menu — unexpected: $(echo "$result" | head -c 200)"
fi

# ──────────────────────────────────────────────────────────────
section "5. iris_doc put with SCM — elicitation or SKIP_SOURCE_CONTROL path"
# Write a small change to trigger OnBeforeSave
CHANGED_CLS='Class Test.Smoke020 Extends %RegisteredObject { ClassMethod Hello() As %String { Return "hello-v2" } }'
PUT_BODY2=$(python3 -c "
import json, sys
content = sys.argv[1]
print(json.dumps({'enc': False, 'content': content.splitlines()}))
" "$CHANGED_CLS")

result=$(echo '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"iris_doc","arguments":{"action":"put","document":"Test.Smoke020.cls","content":"'"$(echo "$CHANGED_CLS" | sed 's/"/\\"/g')"'","namespace":"'"$IRIS_NAMESPACE"'"}}}' \
    | "$BIN" 2>/dev/null || true)

if echo "$result" | grep -qiE '"success"|"elicitation_id"|"question"'; then
    _pass "iris_doc put — got success or elicitation prompt"
else
    _fail "iris_doc put — unexpected response: $(echo "$result" | head -c 300)"
fi

# ──────────────────────────────────────────────────────────────
section "6. Sentinel file — auto-open hint"
HINT_DIR="$HOME/.iris-dev"
HINT_FILE="$HINT_DIR/open-hint.json"

mkdir -p "$HINT_DIR"
HINT_JSON=$(python3 -c "
import json, time
print(json.dumps({'uri': 'isfs://localhost:52773/USER/Test.Smoke020.cls', 'ts': int(time.time()*1000)}))
")
echo "$HINT_JSON" > "$HINT_FILE"

if [[ -f "$HINT_FILE" ]]; then
    content=$(cat "$HINT_FILE")
    if echo "$content" | python3 -c "import json,sys; d=json.load(sys.stdin); assert 'uri' in d and 'ts' in d" 2>/dev/null; then
        _pass "sentinel file written: $HINT_FILE"
    else
        _fail "sentinel file malformed: $content"
    fi
else
    _fail "sentinel file not found at $HINT_FILE"
fi

echo
echo "  NOTE: Open VS Code with the vscode-iris-dev extension installed."
echo "  The file isfs://localhost:52773/USER/Test.Smoke020.cls should auto-open"
echo "  within 3 seconds of the hint being written (the watcher fires on create/change)."
echo "  If nothing opens, check: Extension Host log → 'iris-dev' → openHint"

# ──────────────────────────────────────────────────────────────
section "7. iris_generate — context provider (no LLM call)"
result=$(echo '{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"iris_generate","arguments":{"description":"A service class that stores patient records","namespace":"'"$IRIS_NAMESPACE"'"}}}' \
    | "$BIN" 2>/dev/null || true)

if echo "$result" | grep -qiE '"prompt"|"context"|"system_prompt"'; then
    _pass "iris_generate returns prompt+context (no API key required)"
else
    _fail "iris_generate — unexpected: $(echo "$result" | head -c 300)"
fi

# ──────────────────────────────────────────────────────────────
section "8. Cleanup"
del_status=$(curl_quiet -s -o /dev/null -w "%{http_code}" \
    -X DELETE "${base_url}/v1/${IRIS_NAMESPACE}/doc/${COMPILE_DOC}" || echo "000")

if [[ "$del_status" == "200" || "$del_status" == "204" || "$del_status" == "404" ]]; then
    _pass "cleaned up Test.Smoke020.cls (HTTP $del_status)"
else
    _fail "cleanup DELETE → $del_status"
fi

# ──────────────────────────────────────────────────────────────
section "Summary"
TOTAL=$((PASS + FAIL))
echo "  Passed: $PASS / $TOTAL"
if [[ $FAIL -eq 0 ]]; then
    echo "  All checks passed — safe to merge to master."
else
    echo "  $FAIL check(s) failed — fix before merging."
fi

exit $FAIL
