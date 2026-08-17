#!/bin/sh
# Ensures the agent-friction binary is available.
# Runs on SessionStart; never fails the session.

# Fast path: already installed in plugin data dir from a previous session.
if [ -x "${CLAUDE_PLUGIN_DATA}/bin/agent-friction" ]; then
  exit 0
fi

# Fast path: already on PATH (manual install, cargo install, npm install -g, etc.).
if command -v agent-friction >/dev/null 2>&1; then
  exit 0
fi

# cargo is preferred: compiles from source, installs cleanly into the plugin
# data dir so nothing leaks into the user's global environment.
if command -v cargo >/dev/null 2>&1; then
  echo "agent-friction: installing via cargo (first run — this may take a few minutes)..." >&2
  cargo install \
    --git https://github.com/agent-friction/agent-friction \
    --bin agent-friction \
    --root "${CLAUDE_PLUGIN_DATA}" \
    agent-friction-cli >&2 || true
  exit 0
fi

# Fall back to whichever Node package manager is available.
for pm in npm pnpm yarn bun; do
  if command -v "$pm" >/dev/null 2>&1; then
    echo "agent-friction: installing via $pm (first run)..." >&2
    case "$pm" in
      npm)  npm  install -g @agent-friction/cli >&2 ;;
      pnpm) pnpm add     -g @agent-friction/cli >&2 ;;
      yarn) yarn global add  @agent-friction/cli >&2 ;;
      bun)  bun  add     -g @agent-friction/cli >&2 ;;
    esac || true
    exit 0
  fi
done

echo "agent-friction: could not install automatically. Install the binary manually and restart Claude:" >&2
echo "  cargo install --git https://github.com/agent-friction/agent-friction agent-friction-cli" >&2
echo "  # or: npm install -g @agent-friction/cli" >&2
exit 0
