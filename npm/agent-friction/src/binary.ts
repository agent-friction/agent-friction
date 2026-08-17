import { createRequire } from "node:module";
const require = createRequire(import.meta.url);

const EXE = process.platform === "win32" ? "agent-friction.exe" : "agent-friction";

export function resolveBinary(): string {
	// 1. explicit override: dev loop
	if (process.env.AGENT_FRICTION_BIN) {
		return process.env.AGENT_FRICTION_BIN;
	}

	// 2: the platform package installed alongside the plugin
	try {
		return require.resolve(`agent-friction-${process.platform}-${process.arch}/${EXE}`)
	} catch {}

	// 3: PATH - covers Homebrew / cargo install
	return EXE
}
