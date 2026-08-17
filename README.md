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

### opencode plugin (npm)

Installs the plugin and, via `optionalDependencies`, the prebuilt binary for
your platform:

```sh
npm install agent-friction
```

Then register it in your opencode config:

```json
{
  "plugin": ["agent-friction"]
}
```

Prebuilt binaries ship for darwin-arm64, darwin-x64, linux-x64 and linux-arm64.

### CLI only (cargo)

The plugin does the recording; the CLI does the reporting. If you only want the
CLI, or you're on a platform without a prebuilt binary:

```sh
cargo install --git https://github.com/Cali0707/agent-friction agent-friction-cli
```

That installs an `agent-friction` binary. The plugin will find it on `PATH` if
no platform package is present.

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
