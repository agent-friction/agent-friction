# agent-friction
#
# Three tiers of npm package, and the order between them matters.
#
#   @agent-friction/<platform>  one prebuilt Rust binary, nothing else
#   @agent-friction/cli         the `agent-friction` command; declares the
#                               platform packages as optionalDependencies, so
#                               npm installs only the one matching the host, and
#                               owns resolveBinary() which finds it at runtime
#   @agent-friction/opencode    the host adapter; depends on the cli package
#
# Adapters are per-host and deliberately thin: another one (pi, say) should not
# drag opencode's types into an install that has nothing to do with opencode.
#
#   make local  build for this machine and wire it into the packages
#   make dist   build every binary and stage all npm packages
#   make check  prove a real install works
#   make setup  install the cross-compilation toolchain

SHELL := /usr/bin/env bash
.DEFAULT_GOAL := help

# One source of truth for the version. Packages pin each other to an exact
# version, so drift here is not cosmetic: npm fails to resolve and the plugin
# installs with nothing to run.
VERSION := $(shell sed -n 's/^version = "\([^"]*\)".*/\1/p' Cargo.toml | head -1)

BIN      := agent-friction
CRATE    := agent-friction-cli
DIST     := dist/npm
SCRATCH  := dist/check

# Everything is published under a scope.
SCOPE    := @agent-friction

# The JS packages, in dependency order -- which is also publish order.
CLI      := npm/cli
ADAPTERS := npm/opencode
JS_PKGS  := $(CLI) $(ADAPTERS)

# npm flattens a scope into the tarball filename: @agent-friction/cli packs as
# agent-friction-cli-<version>.tgz.
TARBALL   = $(patsubst @%,%,$(SCOPE))-$(1)-$(VERSION).tgz

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


.PHONY: help setup local link unlink dist publish packages binaries sync-version clean test check

help:
	@echo "agent-friction $(VERSION)"
	@echo
	@echo "  make local            build for this host ($(HOST)) and wire it up"
	@echo "  make link             install into opencode globally (symlinks)"
	@echo "  make unlink           remove the global install"
	@echo "  make setup            install cross-compilation toolchain"
	@echo "  make binaries         build the Rust binary for all platforms"
	@echo "  make build-PLATFORM   build one platform ($(PLATFORMS))"
	@echo "  make packages         build the JS packages ($(JS_PKGS))"
	@echo "  make dist             binaries + packages, staged into $(DIST)"
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
# require.resolve("@agent-friction/<platform>/agent-friction"), and an exports map
# would have to enumerate that subpath to keep it resolvable. Omitting exports
# leaves all subpaths resolvable, which is what esbuild and swc do here too.
package-%: build-%
	@rm -rf $(DIST)/$(BIN)-$*
	@mkdir -p $(DIST)/$(BIN)-$*
	@install -m 755 target/$(TARGET_$*)/release/$(BIN) $(DIST)/$(BIN)-$*/$(BIN)
	@jq -n --arg name "$(SCOPE)/$*" --arg version "$(VERSION)" \
		--arg os "$(word 1,$(subst -, ,$*))" --arg cpu "$(word 2,$(subst -, ,$*))" \
		--arg bin "$(BIN)" \
		'{name:$$name, version:$$version, description:("The agent-friction binary for "+$$os+" "+$$cpu+"."), license:"Apache-2.0", repository:"https://github.com/agent-friction/agent-friction", os:[$$os], cpu:[$$cpu], files:[$$bin], preferUnplugged:true}' \
		> $(DIST)/$(BIN)-$*/package.json
	@printf '# %s\n\nThe precompiled `%s` binary for %s %s.\n\nDo not install this directly. It is an optional dependency of\n[`%s/cli`](https://www.npmjs.com/package/%s/cli), which picks the right one for\nyour platform automatically.\n' \
		"$(SCOPE)/$*" "$(BIN)" "$(word 1,$(subst -, ,$*))" "$(word 2,$(subst -, ,$*))" "$(SCOPE)" "$(SCOPE)" \
		> $(DIST)/$(BIN)-$*/README.md
	@echo "    staged $(DIST)/$(BIN)-$* as $(SCOPE)/$*"

# Rewrites every version and every internal pin from VERSION and PLATFORMS, so
# what a package depends on is always what we just built. The cli package pins
# the platform packages; each adapter pins the cli package.
sync-version:
	@jq --indent 2 --arg v "$(VERSION)" \
		--argjson deps "$$(printf '%s\n' $(PLATFORMS) | jq -R --arg v "$(VERSION)" '{("$(SCOPE)/" + .): $$v}' | jq -s add)" \
		'.version = $$v | .optionalDependencies = $$deps' \
		$(CLI)/package.json > $(CLI)/package.json.tmp
	@mv $(CLI)/package.json.tmp $(CLI)/package.json
	@echo "    version $(VERSION) -> $(CLI)/package.json"
	@for pkg in $(ADAPTERS); do \
		jq --indent 2 --arg v "$(VERSION)" \
			'.version = $$v | .dependencies["$(SCOPE)/cli"] = $$v' \
			$$pkg/package.json > $$pkg/package.json.tmp || exit 1; \
		mv $$pkg/package.json.tmp $$pkg/package.json; \
		echo "    version $(VERSION) -> $$pkg/package.json"; \
	done

# One `bun install` at the workspace root, so the adapters resolve their
# dependency on the cli package from the sibling directory rather than the
# registry -- where, for a version we have not published yet, it does not exist.
# The exact-version pin in package.json is what a published install uses.
packages: sync-version
	@cd npm && bun install
	@for pkg in $(JS_PKGS); do \
		echo "==> $$pkg"; \
		(cd $$pkg && bun run build) || exit 1; \
	done

dist: $(addprefix package-,$(PLATFORMS)) packages
	@echo "dist ready: $(DIST)"

# Dev loop. Builds only the binary this machine can run, then drops its platform
# package into the cli package's node_modules exactly where npm would put it, so
# resolveBinary() resolves it the same way it will in a real install instead of
# silently falling back to PATH.
local: package-$(HOST) packages
	@rm -rf $(CLI)/node_modules/$(SCOPE)/$(HOST)
	@mkdir -p $(CLI)/node_modules/$(SCOPE)
	@cp -R $(DIST)/$(BIN)-$(HOST) $(CLI)/node_modules/$(SCOPE)/$(HOST)
	@echo
	@echo "local build ready ($(HOST))"
	@echo "  plugin:  npm/opencode/dist/plugin.js"
	@echo "  cli:     node $(CLI)/dist/cli.js"
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
	@ln -sfn $(CURDIR)/npm/opencode/dist/plugin.js $(OPENCODE_PLUGINS)/$(BIN).js
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
# directories. Order is the dependency order and it is not optional: a package
# cannot be published before the ones it pins exist on the registry, or the
# first install resolves nothing.
#
# The ./ prefixes are load-bearing. npm reads a bare `npm/cli` as the GitHub
# shorthand for github.com/npm/cli -- the npm CLI itself -- rather than as a
# path, and publishes that instead. A leading ./ forces it to mean the
# directory.
#
# Guarded. A bare `make publish` only rehearses; CONFIRM=1 makes it real.
publish: dist
	@$(if $(CONFIRM),true,echo "DRY RUN -- re-run with CONFIRM=1 to publish for real"; echo)
	@for p in $(PLATFORMS); do \
		npm publish ./$(DIST)/$(BIN)-$$p --access public $(if $(CONFIRM),,--dry-run) || exit 1; \
	done
	@for pkg in $(JS_PKGS); do \
		npm publish ./$$pkg --access public $(if $(CONFIRM),,--dry-run) || exit 1; \
	done

# ---------------------------------------------------------------------------

# Proves the whole chain end to end: optionalDependency selection by os/cpu,
# require.resolve of the binary subpath, and the executable bit surviving the
# round trip. This packs tarballs first on purpose -- `npm install <dir>` links
# the directory and ignores the `files` field, so only a tarball shows what a
# consumer actually receives.
#
# Both install paths get proved, because they fail differently: the cli alone is
# what someone installs for the command, and the adapter is what has to reach
# the binary through a transitive dependency.
check: dist
	@rm -rf $(SCRATCH) && mkdir -p $(SCRATCH)/tarballs $(SCRATCH)/cli $(SCRATCH)/opencode
	@for p in $(PLATFORMS); do \
		(cd $(DIST)/$(BIN)-$$p && npm pack --pack-destination $(CURDIR)/$(SCRATCH)/tarballs >/dev/null) || exit 1; \
	done
	@for pkg in $(JS_PKGS); do \
		(cd $$pkg && npm pack --pack-destination $(CURDIR)/$(SCRATCH)/tarballs >/dev/null) || exit 1; \
	done
	@echo "==> cli alone"
	@cd $(SCRATCH)/cli && npm init -y >/dev/null 2>&1
	@cd $(SCRATCH)/cli && npm install --no-audit --no-fund --loglevel=error \
		$(CURDIR)/$(SCRATCH)/tarballs/$(call TARBALL,$(HOST)) \
		$(CURDIR)/$(SCRATCH)/tarballs/$(call TARBALL,cli)
	@cd $(SCRATCH)/cli && ./node_modules/.bin/$(BIN) --version
	@echo "==> opencode adapter"
	@cd $(SCRATCH)/opencode && npm init -y >/dev/null 2>&1
	@cd $(SCRATCH)/opencode && npm install --no-audit --no-fund --loglevel=error \
		$(CURDIR)/$(SCRATCH)/tarballs/$(call TARBALL,$(HOST)) \
		$(CURDIR)/$(SCRATCH)/tarballs/$(call TARBALL,cli) \
		$(CURDIR)/$(SCRATCH)/tarballs/$(call TARBALL,opencode)
	@cd $(SCRATCH)/opencode && ./node_modules/.bin/$(BIN) --version
	@cd $(SCRATCH)/opencode && node -e "import('@agent-friction/opencode').then(m => { if (!m.AgentFriction) { throw new Error('plugin export missing') } console.log('plugin export ok') })"
	@echo "check ok"

clean:
	cargo clean
	rm -rf dist $(foreach pkg,$(JS_PKGS),$(pkg)/dist)
