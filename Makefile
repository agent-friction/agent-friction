# agent-friction
#
# The npm plugin package ships no binary of its own. It declares one
# optionalDependency per platform, each a small package holding a single
# prebuilt Rust binary, and npm installs only the one whose os/cpu fields match
# the host. resolveBinary() locates it at runtime. These targets build those
# binaries and stage those packages.
#
#   make local  build for this machine and wire it into the plugin
#   make dist   build every binary and stage all npm packages
#   make check  prove a real install works
#   make setup  install the cross-compilation toolchain

SHELL := /usr/bin/env bash
.DEFAULT_GOAL := help

# One source of truth for the version. The plugin pins its platform packages to
# an exact version, so drift here is not cosmetic: npm fails to resolve the
# binary package and the plugin installs with nothing to run.
VERSION := $(shell sed -n 's/^version = "\([^"]*\)".*/\1/p' Cargo.toml | head -1)

BIN      := agent-friction
CRATE    := agent-friction-cli
PLUGIN   := npm/agent-friction
DIST     := dist/npm
SCRATCH  := dist/check

# npm <platform>-<arch>  ->  rust target triple
PLATFORMS := darwin-arm64 darwin-x64 linux-x64 linux-arm64

TARGET_darwin-arm64 := aarch64-apple-darwin
TARGET_darwin-x64   := x86_64-apple-darwin
TARGET_linux-x64    := x86_64-unknown-linux-musl
TARGET_linux-arm64  := aarch64-unknown-linux-musl

# Linux is musl and statically linked. These binaries land on unknown distros
# via npm, so they must not carry a glibc version floor.
#
# rusqlite compiles SQLite from bundled C source, so every target needs a C
# toolchain that can target it. A macOS host already has one for both Darwin
# arches via the Command Line Tools SDK; for musl, cargo-zigbuild borrows zig's.
CARGO_darwin-arm64 := build
CARGO_darwin-x64   := build
CARGO_linux-x64    := zigbuild
CARGO_linux-arm64  := zigbuild

# The host's own npm platform-arch, so the dev loop can build just the one
# binary that machine can actually run.
HOST_OS  := $(if $(filter Darwin,$(shell uname -s)),darwin,linux)
HOST_CPU := $(if $(filter arm64 aarch64,$(shell uname -m)),arm64,x64)
HOST     := $(HOST_OS)-$(HOST_CPU)

# Where `make link` installs for local use.
OPENCODE_PLUGINS := $(HOME)/.config/opencode/plugins
BINDIR           := $(HOME)/.local/bin


.PHONY: help setup local link unlink dist publish plugin binaries sync-version clean test check

help:
	@echo "agent-friction $(VERSION)"
	@echo
	@echo "  make local            build for this host ($(HOST)) and wire it up"
	@echo "  make link             install into opencode globally (symlinks)"
	@echo "  make unlink           remove the global install"
	@echo "  make setup            install cross-compilation toolchain"
	@echo "  make binaries         build the Rust binary for all platforms"
	@echo "  make build-PLATFORM   build one platform ($(PLATFORMS))"
	@echo "  make plugin           build the npm plugin package (JS + types)"
	@echo "  make dist             binaries + plugin, staged into $(DIST)"
	@echo "  make check            install into a scratch project and run the CLI"
	@echo "  make publish          publish every package to npm"
	@echo "  make test             cargo test"
	@echo "  make clean            remove build output"

# ---------------------------------------------------------------------------
# toolchain
# ---------------------------------------------------------------------------

setup:
	rustup target add $(foreach p,$(PLATFORMS),$(TARGET_$(p)))
	@command -v zig >/dev/null || { echo "zig not found: brew install zig"; exit 1; }
	@command -v jq >/dev/null || { echo "jq not found: brew install jq"; exit 1; }
	@command -v cargo-zigbuild >/dev/null || cargo install cargo-zigbuild
	@echo "toolchain ready"

# ---------------------------------------------------------------------------
# rust binaries
# ---------------------------------------------------------------------------

binaries: $(addprefix build-,$(PLATFORMS))

build-%:
	@echo "==> $* ($(TARGET_$*))"
	cargo $(CARGO_$*) --release -p $(CRATE) --target $(TARGET_$*)

test:
	cargo test --workspace

# ---------------------------------------------------------------------------
# npm packages
# ---------------------------------------------------------------------------

# A platform package is just the binary plus a package.json. It deliberately has
# no "exports" field: resolveBinary() finds the binary with
# require.resolve("agent-friction-<platform>/agent-friction"), and an exports map
# would have to enumerate that subpath to keep it resolvable. Omitting exports
# leaves all subpaths resolvable, which is what esbuild and swc do here too.
package-%: build-%
	@rm -rf $(DIST)/$(BIN)-$*
	@mkdir -p $(DIST)/$(BIN)-$*
	@install -m 755 target/$(TARGET_$*)/release/$(BIN) $(DIST)/$(BIN)-$*/$(BIN)
	@jq -n --arg name "$(BIN)-$*" --arg version "$(VERSION)" \
		--arg os "$(word 1,$(subst -, ,$*))" --arg cpu "$(word 2,$(subst -, ,$*))" \
		--arg bin "$(BIN)" \
		'{name:$$name, version:$$version, description:("The agent-friction binary for "+$$os+" "+$$cpu+"."), license:"Apache-2.0", repository:"https://github.com/Cali0707/agent-friction", os:[$$os], cpu:[$$cpu], files:[$$bin], preferUnplugged:true}' \
		> $(DIST)/$(BIN)-$*/package.json
	@printf '# %s\n\nThe precompiled `%s` binary for %s %s.\n\nDo not install this directly. It is an optional dependency of\n[`%s`](https://www.npmjs.com/package/%s), which picks the right one for your\nplatform automatically.\n' \
		"$(BIN)-$*" "$(BIN)" "$(word 1,$(subst -, ,$*))" "$(word 2,$(subst -, ,$*))" "$(BIN)" "$(BIN)" \
		> $(DIST)/$(BIN)-$*/README.md
	@echo "    staged $(DIST)/$(BIN)-$*"

# Rewrites the plugin's version and optionalDependencies from VERSION and
# PLATFORMS, so the packages it pins are always the ones we just built.
sync-version:
	@jq --indent 2 --arg v "$(VERSION)" \
		--argjson deps "$$(printf '%s\n' $(PLATFORMS) | jq -R --arg v "$(VERSION)" '{("$(BIN)-" + .): $$v}' | jq -s add)" \
		'.version = $$v | .optionalDependencies = $$deps' \
		$(PLUGIN)/package.json > $(PLUGIN)/package.json.tmp
	@mv $(PLUGIN)/package.json.tmp $(PLUGIN)/package.json
	@echo "    version $(VERSION) -> $(PLUGIN)/package.json"

plugin: sync-version
	@echo "==> plugin"
	cd $(PLUGIN) && bun install && bun run build

dist: $(addprefix package-,$(PLATFORMS)) plugin
	@echo "dist ready: $(DIST)"

# Dev loop. Builds only the binary this machine can run, then drops its platform
# package into the plugin's node_modules exactly where npm would put it, so
# resolveBinary() resolves it the same way it will in a real install instead of
# silently falling back to PATH.
local: package-$(HOST) plugin
	@rm -rf $(PLUGIN)/node_modules/$(BIN)-$(HOST)
	@mkdir -p $(PLUGIN)/node_modules
	@cp -R $(DIST)/$(BIN)-$(HOST) $(PLUGIN)/node_modules/$(BIN)-$(HOST)
	@echo
	@echo "local build ready ($(HOST))"
	@echo "  plugin:  $(PLUGIN)/dist/plugin.js"
	@echo "  cli:     node $(PLUGIN)/dist/cli.js"
	@echo "  binary:  target/$(TARGET_$(HOST))/release/$(BIN)"

# Installs into opencode globally, as symlinks so a rebuild takes effect without
# reinstalling.
#
# Deliberately not via node_modules in the config dir: opencode runs `bun
# install` there at startup and would clobber it. The plugin goes in the global
# plugins directory, which opencode loads directly, and the binary goes on PATH
# -- the fallback resolveBinary() already provides for cargo/Homebrew installs.
link: local
	@mkdir -p $(OPENCODE_PLUGINS) $(BINDIR)
	@ln -sfn $(CURDIR)/$(PLUGIN)/dist/plugin.js $(OPENCODE_PLUGINS)/$(BIN).js
	@ln -sfn $(CURDIR)/target/$(TARGET_$(HOST))/release/$(BIN) $(BINDIR)/$(BIN)
	@echo
	@echo "linked into opencode:"
	@echo "  $(OPENCODE_PLUGINS)/$(BIN).js"
	@echo "  $(BINDIR)/$(BIN)"
	@echo
	@echo "restart opencode to load it"

unlink:
	@rm -f $(OPENCODE_PLUGINS)/$(BIN).js $(BINDIR)/$(BIN)
	@echo "unlinked"

# ---------------------------------------------------------------------------
# release
# ---------------------------------------------------------------------------

# npm publish packs the tarball itself, so these publish straight from their
# directories. Order matters: the platform packages must exist on the registry
# before the plugin that pins them, or the first install resolves no binary.
#
# Guarded. A bare `make publish` only rehearses; CONFIRM=1 makes it real.
publish: dist
	@$(if $(CONFIRM),true,echo "DRY RUN -- re-run with CONFIRM=1 to publish for real"; echo)
	@for p in $(PLATFORMS); do \
		npm publish $(DIST)/$(BIN)-$$p --access public $(if $(CONFIRM),,--dry-run) || exit 1; \
	done
	npm publish $(PLUGIN) --access public $(if $(CONFIRM),,--dry-run)

# ---------------------------------------------------------------------------

# Proves the whole chain end to end: optionalDependency selection by os/cpu,
# require.resolve of the binary subpath, and the executable bit surviving the
# round trip. This packs tarballs first on purpose -- `npm install <dir>` links
# the directory and ignores the `files` field, so only a tarball shows what a
# consumer actually receives.
check: dist
	@rm -rf $(SCRATCH) && mkdir -p $(SCRATCH)/tarballs $(SCRATCH)/project
	@for p in $(PLATFORMS); do \
		(cd $(DIST)/$(BIN)-$$p && npm pack --pack-destination $(CURDIR)/$(SCRATCH)/tarballs >/dev/null) || exit 1; \
	done
	@(cd $(PLUGIN) && npm pack --pack-destination $(CURDIR)/$(SCRATCH)/tarballs >/dev/null)
	@cd $(SCRATCH)/project && npm init -y >/dev/null 2>&1
	@cd $(SCRATCH)/project && npm install --no-audit --no-fund --loglevel=error \
		$(CURDIR)/$(SCRATCH)/tarballs/$(BIN)-$(HOST)-$(VERSION).tgz \
		$(CURDIR)/$(SCRATCH)/tarballs/$(BIN)-$(VERSION).tgz
	@cd $(SCRATCH)/project && ./node_modules/.bin/$(BIN) --version
	@echo "check ok"

clean:
	cargo clean
	rm -rf dist $(PLUGIN)/dist
