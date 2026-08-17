# agent-friction

Records every permission prompt you answer and every tool call that fails, then
tells you which approvals you keep granting by hand.

You approve `tail -15`, then `tail -20`, then `tail -60`. agent-friction notices
those are the same decision three times, folds them into `tail *`, and — once
the evidence is there and nothing in that family was ever denied — suggests you
allow-list it. Fewer prompts, and the ones left are the ones worth reading.

```
┌──────┬────────────┬──────────────┬────────┬──────────┐
│ Tool ┆ Pattern    ┆ Verdict      ┆ Events ┆ Evidence │
╞══════╪════════════╪══════════════╪════════╪══════════╡
│ bash ┆ tail *     ┆ allow (72%)  ┆ 41     ┆ tail -15 │
│      ┆            ┆              ┆        ┆ tail -20 │
│      ┆            ┆              ┆        ┆ tail -60 │
└──────┴────────────┴──────────────┴────────┴──────────┘
```

Nothing leaves your machine. Everything lands in a local SQLite database.

Failures are recorded for the same reason: the errors your agent hits over and
over are a map of where your setup fights it — a missing tool, a command that
never works in this repo, a path it keeps guessing wrong. Right now that's
reported as counts of similar errors; teaching `analyze` to make
recommendations from it is the obvious next step.

## Install

[Claude Code plugin](#claude-code-plugin) · [CLI only](#cli-only) · [opencode plugin](#opencode-plugin)

### Claude Code plugin

Add the marketplace and install the plugin:

```sh
/plugin marketplace add github:agent-friction/agent-friction
/plugin install agent-friction@agent-friction
```

The plugin installs `agent-friction` automatically on first session start — via
cargo if it's on your `PATH`, otherwise via the first Node package manager it
finds. You can skip the wait by installing the binary yourself first (see [CLI
only](#cli-only) below) before enabling the plugin.

### CLI only

The adapter does the recording; the CLI does the reporting. If you want the CLI
on its own:

```sh
npm install -g @agent-friction/cli
```

or from source, which also covers platforms without a prebuilt binary:

```sh
cargo install --git https://github.com/agent-friction/agent-friction agent-friction-cli
```

Both install an `agent-friction` binary. The plugin falls back to `PATH`, so a
cargo install is enough for it to work.

### opencode plugin

```sh
opencode plugin @agent-friction/opencode
```

That installs the plugin and adds it to your project config; pass `-g` for your
global config. It brings `@agent-friction/cli` with it, which pulls the prebuilt
binary for your platform and puts the `agent-friction` command on your path.

Prebuilt binaries ship for darwin-arm64, darwin-x64, linux-x64 and linux-arm64.

## Usage

```sh
agent-friction stats                     # what happened
agent-friction stats permissions         # just the prompts
agent-friction stats failures            # just the tool failures
agent-friction analyze                   # what to do about it
```

Useful flags, on both commands:

| Flag | Meaning |
| --- | --- |
| `--since` | How far back to look. Accepts `2026-01-01` or `last tuesday`. Defaults to two weeks. |
| `--repo` | Restrict to one repository. |
| `--limit` | Show only the busiest N rows. |
| `--min-count` | Hide anything seen fewer than N times. |
| `--db` | Use a different database file. |

`analyze` also takes `--json`, which emits the raw suggestions with the full
evidence for each.

### Reading the verdicts

- **allow** — enough approvals, no denials. Worth allow-listing.
- **keep asking** — denied at least once. A single denial anywhere in a family
  blocks the whole family, so `git push --force` being refused once keeps `git
  push *` off the table.
- **insufficient data** — not enough evidence yet. Most rows start here.

A pattern is only widened when at least three distinct commands support it, so
`git status --short` alone never becomes `git status *`. The Evidence column
shows exactly which observations a widened rule rests on.

## Packages

| Package | What it is |
| --- | --- |
| [`claude/`](claude) | The Claude Code plugin. Records prompts and failures via hooks. |
| [`@agent-friction/cli`](npm/cli) | The `agent-friction` command, and the binary resolution adapters share. |
| `@agent-friction/{darwin,linux}-{arm64,x64}` | One prebuilt binary each. Installed automatically. |
| [`@agent-friction/opencode`](npm/opencode) | The opencode plugin. Records prompts and failures. |
| [`agent-friction-cli`](crates/cli) / [`agent-friction-core`](crates/core) | The Rust crates behind the binary. |

Adapters are per-host and deliberately thin, so a future one does not drag
opencode's dependencies into an install that has nothing to do with opencode.

## Development

```sh
make test     # cargo test --workspace
make local    # build for this machine and wire it into the plugin
make link     # install into opencode globally, as symlinks
make check    # pack real tarballs and prove a fresh install works
make help     # everything else
```

`make link` symlinks the plugin and binary, so a rebuild takes effect without
reinstalling. Restart opencode to pick it up.

## License

Apache-2.0
