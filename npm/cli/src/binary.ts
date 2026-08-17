import { createRequire } from "node:module";
const require = createRequire(import.meta.url);

const EXE = process.platform === "win32" ? "agent-friction.exe" : "agent-friction";

/**
 * Locates the Rust binary.
 *
 * This lives in `@agent-friction/cli` because that is the package declaring the
 * platform binaries as optionalDependencies, and `require.resolve` searches
 * from *this* file. Adapters must import it rather than inline a copy: bundled
 * into an adapter's own dist, the resolution would start from the wrong place
 * and fall through to PATH.
 */
export function resolveBinary(): string {
	// 1. explicit override: dev loop
	if (process.env.AGENT_FRICTION_BIN) {
		return process.env.AGENT_FRICTION_BIN;
	}

	// 2: the platform package installed alongside this one
	try {
		return require.resolve(`@agent-friction/${process.platform}-${process.arch}/${EXE}`)
	} catch {}

	// 3: PATH - covers Homebrew / cargo install
	return EXE
}
