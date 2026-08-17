# agent-friction

Records where an agent gets stuck — the permission prompts you answer and the
tool calls that fail — so friction can be measured instead of guessed at.

This package is the opencode plugin.

## Install

```jsonc
// opencode.jsonc
{ "plugin": ["agent-friction"] }
```

The matching platform binary is pulled in as an optional dependency, so there is
nothing else to install.

## What it does

Every permission prompt you answer is recorded against the tool and pattern it
was asked for, along with your decision. Every failed tool call is recorded with
its error and input.

Over time that builds a picture of which permission rules you approve so
routinely that they are worth allow-listing outright, and which tools fail often
enough to be worth fixing.

Recording is best-effort and never throws, so it cannot break a session.
