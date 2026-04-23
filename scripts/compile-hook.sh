#!/usr/bin/env bash
set -euo pipefail

INPUT=$(cat)

EVENT=$(printf '%s' "$INPUT" | jq -r '.hook_event_name // "PostToolUse"')

if [[ "$EVENT" == "FileChanged" ]]; then
    [[ "${IRIS_COMPILE_ON_SAVE:-}" != "true" ]] && exit 0
    FILE_PATH=$(printf '%s' "$INPUT" | jq -r '.file_path // empty')
else
    FILE_PATH=$(printf '%s' "$INPUT" | jq -r '.tool_input.file_path // empty')
fi

[[ -z "$FILE_PATH" ]] && exit 0

EXT="${FILE_PATH##*.}"
case "$EXT" in
    cls|mac|inc) ;;
    *) exit 0 ;;
esac

[[ "${IRIS_AUTO_COMPILE:-}" == "false" ]] && exit 0

if [[ -z "${IRIS_HOST:-}" || -z "${IRIS_WEB_PORT:-}" ]]; then
    echo "IRIS not connected — set IRIS_HOST, IRIS_WEB_PORT, IRIS_USERNAME, IRIS_PASSWORD"
    exit 0
fi

BASENAME=$(basename "$FILE_PATH")
DOC_NAME="$BASENAME"

CLASS_NAME="${BASENAME%.*}"

if [[ "$EXT" == "cls" ]]; then
    PARENT=$(basename "$(dirname "$FILE_PATH")")
    if [[ "$PARENT" != "." && "$PARENT" != "" && "$PARENT" != "workspace" && "$PARENT" != "/" ]]; then
        DOC_NAME="${PARENT}.${BASENAME}"
        CLASS_NAME="${PARENT}.${CLASS_NAME}"
    fi
fi

BASE_URL="http://${IRIS_HOST}:${IRIS_WEB_PORT}/api/atelier/v1"
NS="${IRIS_NAMESPACE:-USER}"
USER="${IRIS_USERNAME:-_SYSTEM}"
PASS="${IRIS_PASSWORD:-SYS}"

START=$(date +%s%N 2>/dev/null || echo 0)

RESPONSE=$(curl --max-time 3 -s \
    -X POST \
    -u "${USER}:${PASS}" \
    -H "Content-Type: application/json" \
    "${BASE_URL}/${NS}/action/compile" \
    -d "[\"${DOC_NAME}\"]" 2>/dev/null) || {
    echo "IRIS not connected — set IRIS_HOST, IRIS_WEB_PORT, IRIS_USERNAME, IRIS_PASSWORD"
    exit 0
}

HTTP_CODE=$(curl --max-time 3 -s -o /dev/null -w "%{http_code}" \
    -u "${USER}:${PASS}" \
    "${BASE_URL}/${NS}/action/compile" \
    -X POST \
    -H "Content-Type: application/json" \
    -d "[\"${DOC_NAME}\"]" 2>/dev/null) || HTTP_CODE="000"

BODY="$RESPONSE"

if [[ "$HTTP_CODE" == "000" || -z "$HTTP_CODE" ]]; then
    echo "IRIS not connected — set IRIS_HOST, IRIS_WEB_PORT, IRIS_USERNAME, IRIS_PASSWORD"
    exit 0
fi

END=$(date +%s%N 2>/dev/null || echo 0)
if [[ "$START" != "0" && "$END" != "0" ]]; then
    ELAPSED_MS=$(( (END - START) / 1000000 ))
    ELAPSED_S=$(awk "BEGIN {printf \"%.1f\", $ELAPSED_MS / 1000}")
else
    ELAPSED_S="?"
fi

ERRORS=$(printf '%s' "$BODY" | jq -r '
    (.status.errors[]?.error // empty),
    (.result.console[]? | select(startswith("ERROR") or startswith(" ERROR")))
' 2>/dev/null | grep -v "^$" | head -10 || true)

if [[ -z "$ERRORS" ]]; then
    echo "Compiled ${CLASS_NAME} OK (${ELAPSED_S}s)"
else
    echo "Compile errors in ${CLASS_NAME}:"
    printf '%s\n' "$ERRORS" | while IFS= read -r line; do
        echo "  $line"
    done
fi
