#!/usr/bin/env bash
set -euo pipefail

INPUT=$(cat)
TEXT=$(echo "$INPUT" | jq -r '.text // empty')

if [ -z "$TEXT" ]; then
    echo '{"status":"error","data":null,"error":"Missing required field: text"}'
    exit 0
fi

REVERSED=$(echo "$TEXT" | rev)

echo "{\"status\":\"success\",\"data\":\"$REVERSED\",\"error\":null}"
