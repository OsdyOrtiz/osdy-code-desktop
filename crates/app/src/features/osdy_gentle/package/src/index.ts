/**
 * osdy-gentle: Osdy-Pi Codex accounts and profiles for Gentle Shell desktop.
 * Registers `/osdy-profile`, `/osdy-account`, `/osdy-auth-status`, and `/osdy-usage`.
 */
import type { CodexUsageSnapshot } from "./codex-usage.ts";
import { translator } from "./i18n.ts";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { listProfiles, withAccountSwitchFence } from "./account-store.ts";
import { createCodexResetActions } from "./codex-reset.ts";
import { osdyAccountCommand, osdyProfileCommand, osdyUsageCommand, type CommandContext, type Deps } from "./commands.ts";
import type { EventBus } from "./gentle-profiles.ts";
import { describePiRuntimeAuth, resolvePiRuntimeAuth } from "./pi-runtime-auth.ts";
import { registerProfileWidget, type WidgetHost } from "./profile-widget.ts";

/** Same rule as Pi's getAgentDir(): PI_CODING_AGENT_DIR (with ~), else ~/.pi/agent. */
export function agentDir(env: NodeJS.ProcessEnv = process.env): string {
	const value = env.PI_CODING_AGENT_DIR;
	if (value) return value === "~" ? homedir() : value.startsWith("~/") ? join(homedir(), value.slice(2)) : resolve(value);
	return join(homedir(), ".pi", "agent");
}

interface HostSdk {
	VERSION: string;
	getAgentDir(): string;
}

interface PiLike extends WidgetHost {
	events: EventBus;
	registerCommand(
		name: string,
		options: {
			description?: string;
			getArgumentCompletions?: (prefix: string) => Promise<{ value: string; label: string }[] | null>;
			handler: (args: string, ctx: CommandContext) => Promise<void>;
		},
	): void;
}

export default function osdyGentle(pi: PiLike): void {
	const t = translator();
	let session = new AbortController();
	let usageRun: string | undefined;
	// Only snapshots from quota lookups requested by commands live here.
	const quotas = new Map<string, Map<string, CodexUsageSnapshot>>();
	const stopSession = () => {
		session.abort();
		usageRun = undefined;
	};
	pi.on?.("session_start", () => {
		stopSession();
		session = new AbortController();
	});
	pi.on?.("session_shutdown", stopSession);

	let host: Promise<HostSdk | undefined> | undefined;
	const getHost = () => (host ??= import("@earendil-works/pi-coding-agent").catch(() => undefined));
	const hostAgentDir = (runtime: HostSdk | undefined) => resolve(runtime?.getAgentDir() ?? agentDir());
	const currentAgentDir = async () => hostAgentDir(await getHost());
	// Lookups of one account can overlap (a retried "View usage"); each takes a
	// ticket when it starts, and an answer older than the one already recorded
	// is dropped, so a slow stale response never overwrites a fresher one.
	const tickets = new Map<string, { started: number; recorded: number }>();
	const startQuota = (directory: string, signal: AbortSignal, account: string) => {
		const id = `${directory}\0${account}`;
		let ticket = tickets.get(id);
		if (!ticket) tickets.set(id, ticket = { started: 0, recorded: 0 });
		const mine = ++ticket.started;
		return (snapshot: CodexUsageSnapshot): boolean => {
			if (signal.aborted || mine <= ticket.recorded) return false;
			ticket.recorded = mine;
			let known = quotas.get(directory);
			if (!known) quotas.set(directory, known = new Map());
			known.set(account, snapshot);
			return true;
		};
	};
	const deps = async (ctx: CommandContext): Promise<Deps> => {
		const signal = session.signal;
		const runtime = await getHost();
		const directory = await currentAgentDir();
		const base = {
			agentDir: directory,
			events: pi.events,
			startQuota(account: string) {
				const record = startQuota(directory, signal, account);
				return (snapshot: CodexUsageSnapshot) => { record(snapshot); };
			},
		};
		const binding = resolvePiRuntimeAuth(runtime?.VERSION, ctx, join(directory, "auth.json"));
		if (!binding) return base;
		return {
			...base,
			authBackend: binding.backend,
			resetActions: createCodexResetActions({
				agentDir: directory,
				ui: ctx.ui,
				signal,
				readActiveAuth: () => binding.readActiveAuth(signal),
				withCurrentAuthForDispatch: (dispatch) => binding.withCurrentAuthForDispatch(dispatch, signal),
				// Guard extension switches separately from native credential writes.
				withAccountSwitchFence: (run) => withAccountSwitchFence(directory, run),
			}),
		};
	};
	const refreshWidget = registerProfileWidget(
		pi, () => resolve(agentDir()), undefined,
		(directory, account) => quotas.get(directory)?.get(account),
		() => usageRun,
	);

	pi.registerCommand("osdy-profile", {
		description: t("Switch Osdy profile: Codex account (all tabs) + linked Gentle model profile"),
		getArgumentCompletions: async (prefix) => {
			const names = await listProfiles(await currentAgentDir()).catch(() => [] as string[]);
			const items = names.filter((n) => n.toLowerCase().startsWith(prefix.toLowerCase())).map((n) => ({ value: n, label: n }));
			return items.length ? items : null;
		},
		handler: async (args, ctx) => osdyProfileCommand(args, ctx, await deps(ctx)),
	});

	pi.registerCommand("osdy-account", {
		description: t("Manage Osdy Codex accounts: switch, link model profile, quota, banked resets, save, add, rename, remove, import"),
		handler: async (args, ctx) => osdyAccountCommand(args, ctx, await deps(ctx)),
	});

	pi.registerCommand("osdy-auth-status", {
		description: t("Report Pi auth owner compatibility without reading credentials or testing the lock"),
		handler: async (_args, ctx) => {
			const runtime = await getHost();
			// Pi's loader may expose SDK exports as getters. Read VERSION from this
			// trusted import exactly as the switch does, not as an own data property.
			const sdk = runtime === undefined ? undefined : { VERSION: runtime.VERSION };
			const status = describePiRuntimeAuth(sdk, ctx, () => join(hostAgentDir(runtime), "auth.json"));
			ctx.ui.notify(status, "info");
		},
	});

	pi.registerCommand("osdy-usage", {
		description: t("View Codex quota without switching accounts: /osdy-usage <name> | --all [--run <id>]"),
		handler: async (args, ctx) => {
			const signal = session.signal;
			const directory = await currentAgentDir();
			let refresh: Promise<void> | undefined;
			const completedRun = await osdyUsageCommand(args, ctx, {
				agentDir: directory,
				events: pi.events,
				startQuota(account) {
					const record = startQuota(directory, signal, account);
					return (snapshot) => { if (record(snapshot)) refresh = refreshWidget(); };
				},
			});
			await refresh;
			if (completedRun !== undefined && !signal.aborted) {
				usageRun = completedRun;
				await refreshWidget();
			}
		},
	});
}
