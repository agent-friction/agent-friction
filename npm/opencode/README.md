# @agent-friction/opencode

The [agent-friction](https://github.com/agent-friction/agent-friction) plugin for
opencode.

Records where an agent gets stuck — the permission prompts you answer and the
tool calls that fail — so friction can be measured instead of guessed at.

## Install

```sh
opencode plugin @agent-friction/opencode
```

That installs the plugin and adds it to your config. Pass `-g` to install into
your global config instead of the current project:

```sh
opencode plugin -g @agent-friction/opencode
```

`@agent-friction/cli` comes with it, which brings the binary for your platform
and puts the `agent-friction` command on your path.

## What it does

Every permission prompt you answer is recorded against the tool and pattern it
was asked for, along with your decision. Every failed tool call is recorded with
its error and input.

Over time that builds a picture of which permission rules you approve so
routinely that they are worth allow-listing outright, and which tools fail often
enough to be worth fixing. Ask it with:

```sh
agent-friction analyze
```

Recording is best-effort and never throws, so it cannot break a session.
