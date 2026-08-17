#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { resolveBinary } from "./binary";

const r = spawnSync(resolveBinary(), process.argv.slice(2), { stdio: "inherit" });

if (r.error) {
	console.error(`agent-friction: ${r.error.message}`);
	process.exit(1);
}

process.exit(r.status ?? 1);
