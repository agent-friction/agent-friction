#!/bin/sh
# PostToolUseFailure: log the tool failure.
# Never fails the session.

# Resolve binary: plugin data dir first, then PATH.
AF_BIN="${CLAUDE_PLUGIN_DATA}/bin/agent-friction"
if ! [ -x "$AF_BIN" ]; then
  AF_BIN="agent-friction"
fi
command -v "$AF_BIN" >/dev/null 2>&1 || exit 0

input=$(cat)

session_id=$(printf '%s' "$input" | jq -r '.session_id // ""')
tool_name=$(printf '%s' "$input" | jq -r '.tool_name // ""')
tool_input=$(printf '%s' "$input" | jq -c '.tool_input // {}')
cwd=$(printf '%s' "$input" | jq -r '.cwd // ""')

error=$(printf '%s' "$input" | jq -r '
  .tool_result |
  if type == "object" then .error // .message // (. | tostring)
  else (. | tostring)
  end
' 2>/dev/null) || error="unknown error"

"$AF_BIN" log failure \
  --agent claude \
  --session-id "$session_id" \
  --repo "$cwd" \
  --tool "$tool_name" \
  --error "$error" \
  --input "$tool_input" \
  >/dev/null 2>&1 || true

exit 0
