import type { Plugin } from "@opencode-ai/plugin";
import type { Event } from "@opencode-ai/sdk/v2/types";
import { resolveBinary } from "@agent-friction/cli";

/**
 * The server runs two independent permission stacks with separate pending
 * state: v1 (`permission.asked`) backs the session tools, v2
 * (`permission.v2.asked`) backs the core tools -- bash, edit, write, read,
 * grep, glob. There is no bridge between them, so a prompt raises exactly one
 * event of one version and both families must be handled to see everything.
 *
 * Events are typed against the v2 SDK union, which declares both families. The
 * plugin hook is typed with the v1 `Event`, which declares a
 * `permission.updated` event the server never emits, so we widen once here.
 */

const DECISIONS = {
	once: "allow_once",
	always: "allow_always",
	reject: "deny",
} as const;

type Reply = keyof typeof DECISIONS;

const AGENT = "opencode";

/**
 * Answering one prompt settles others that were pending alongside it, and the
 * server publishes a full reply event for each: "reject" force-rejects every
 * other pending prompt in the session, "always" auto-approves the ones the
 * newly saved rule now covers. Only "once" settles nothing but itself.
 *
 * Those synthetic replies are not user decisions. Recording them would be
 * actively harmful for "reject", because a single deny row pins a
 * (tool, pattern) to KeepAsking permanently -- one rejection would burn every
 * pattern that happened to be queued behind it.
 *
 * The reply event carries no marker distinguishing synthetic from real, but it
 * does not need to: we already know which prompts are outstanding, so on a
 * cascading reply we drop the rest of the session's pending prompts up front.
 * Their synthetic replies then arrive to find no matching ask and are ignored.
 *
 * This is exact for "reject" and best-effort for "always" -- we cannot evaluate
 * pattern coverage here, so we drop the session's other prompts rather than
 * work out which the new rule actually covers. An uncovered prompt stays
 * pending and its real answer goes unrecorded. That errs toward recording less
 * than happened, never more, which is the safe direction.
 */
const CASCADES = new Set<Reply>(["always", "reject"]);

/** A pending prompt: the permission category and the rules it covers. */
type Ask = { sessionID: string; tool: string; patterns: string[] };

export const AgentFriction: Plugin = async ({ $, worktree, client }) => {
	const bin = resolveBinary();

	// A reply names only a requestID; the tool and patterns come from the ask.
	const asks = new Map<string, Ask>();
	// A v2 failure names only a callID; the tool and input come from the call.
	const calls = new Map<string, { sessionID: string; tool: string; input: unknown }>();

	/** Never throws: telemetry must not break the host session. */
	const record = async (args: string[]) => {
		const { exitCode, stderr } = await $`${bin} ${args}`.quiet().nothrow();
		if (exitCode === 0) return;
		await client.app
			.log({
				body: {
					service: "agent-friction",
					level: "error",
					message: "agent-friction exited non-zero",
					extra: { exitCode, stderr: stderr.toString().trim(), args },
				},
			})
			.catch(() => {});
	};

	const recordPermission = (ask: Ask, decision: string) =>
		record([
			"log", "permission",
			"--agent", AGENT,
			"--session-id", ask.sessionID,
			"--repo", worktree,
			"--tool", ask.tool,
			// One row per pattern: the analysis groups by (tool, pattern) to judge
			// whether an individual rule is safe to allow-list.
			...ask.patterns.flatMap((pattern) => ["--pattern", pattern]),
			"--decision", decision,
		]);

	const recordFailure = (sessionID: string, tool: string, error: string, input: unknown) =>
		record([
			"log", "failure",
			"--agent", AGENT,
			"--session-id", sessionID,
			"--repo", worktree,
			"--tool", tool,
			"--error", error,
			"--input", JSON.stringify(input ?? null),
		]);

	/** Drops every pending prompt for a session. */
	const forgetSession = (sessionID: string) => {
		for (const [requestID, ask] of asks) {
			if (ask.sessionID === sessionID) asks.delete(requestID);
		}
	};

	const reply = async (requestID: string, reply: Reply) => {
		const ask = asks.get(requestID);
		// A prompt raised before this plugin loaded, or a synthetic reply for a
		// prompt we already dropped as a cascade.
		if (!ask) return;
		asks.delete(requestID);

		if (CASCADES.has(reply)) forgetSession(ask.sessionID);

		await recordPermission(ask, DECISIONS[reply]);
	};

	return {
		event: async ({ event }) => {
			const next = event as unknown as Event;
			switch (next.type) {
				case "permission.asked": {
					const { id, sessionID, permission, patterns } = next.properties;
					asks.set(id, { sessionID, tool: permission, patterns });
					return;
				}

				case "permission.v2.asked": {
					const { id, sessionID, action, resources } = next.properties;
					asks.set(id, { sessionID, tool: action, patterns: resources });
					return;
				}

				case "permission.replied":
				case "permission.v2.replied":
					await reply(next.properties.requestID, next.properties.reply);
					return;

				case "message.part.updated": {
					const { part } = next.properties;
					if (part.type !== "tool" || part.state.status !== "error") return;
					// Cancellations and permission denials are reported as tool
					// errors; neither is a tool failure.
					if (part.state.metadata?.interrupted) return;
					await recordFailure(
						part.sessionID,
						part.tool,
						part.state.error,
						part.state.input,
					);
					return;
				}

				case "session.next.tool.called":
					calls.set(next.properties.callID, {
						sessionID: next.properties.sessionID,
						tool: next.properties.tool,
						input: next.properties.input,
					});
					return;

				case "session.next.tool.success":
					calls.delete(next.properties.callID);
					return;

				case "session.next.tool.failed": {
					const { sessionID, callID, error } = next.properties;
					const call = calls.get(callID);
					if (!call) return;
					calls.delete(callID);
					await recordFailure(sessionID, call.tool, error.message, call.input);
					return;
				}

				// Aborting a session settles its pending prompts silently, with no
				// reply event, and in-flight calls never report success or failure.
				// Nothing else would ever evict those entries.
				case "session.deleted": {
					const { sessionID } = next.properties;
					forgetSession(sessionID);
					for (const [callID, call] of calls) {
						if (call.sessionID === sessionID) calls.delete(callID);
					}
					return;
				}
			}
		},
	};
};
