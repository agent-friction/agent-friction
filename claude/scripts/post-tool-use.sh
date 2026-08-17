#!/bin/sh
# PostToolUse: log the tool use as an allow_once permission event.
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

# Derive the most meaningful single string from the tool input as the pattern.
pattern=$(printf '%s' "$input" | jq -r '
  .tool_input |
  if   .command   then .command
  elif .file_path then .file_path
  elif .path      then .path
  elif .pattern   then .pattern
  else                 (. | tostring)
  end
' 2>/dev/null) || pattern="$tool_input"

"$AF_BIN" log permission \
  --agent claude \
  --session-id "$session_id" \
  --repo "$cwd" \
  --tool "$tool_name" \
  --pattern "$pattern" \
  --decision allow_once \
  >/dev/null 2>&1 || true

exit 0
