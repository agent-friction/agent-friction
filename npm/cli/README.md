# @agent-friction/cli

The `agent-friction` command line tool.

Records the permission prompts you answer and the tool calls that fail, then
tells you which approvals you keep granting by hand. Nothing leaves your
machine.

## Install

```sh
npm install -g @agent-friction/cli
```

Or without installing:

```sh
npx @agent-friction/cli stats
```

The binary for your platform is pulled in as an optional dependency, so there is
nothing else to install. Prebuilt binaries ship for darwin-arm64, darwin-x64,
linux-x64 and linux-arm64; on anything else, install the Rust crate instead and
this package will find it on `PATH`.

## Usage

```sh
agent-friction stats      # what happened
agent-friction analyze    # what to do about it
agent-friction --help
```

## Recording

This package only reports. The recording is done by a host adapter — for
opencode, that is [`@agent-friction/opencode`](https://www.npmjs.com/package/@agent-friction/opencode).
