#!/bin/sh
# PermissionDenied: log the denial.
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
' 2>/dev/null) || pattern=""

"$AF_BIN" log permission \
  --agent claude \
  --session-id "$session_id" \
  --repo "$cwd" \
  --tool "$tool_name" \
  --pattern "$pattern" \
  --decision deny \
  >/dev/null 2>&1 || true

exit 0
