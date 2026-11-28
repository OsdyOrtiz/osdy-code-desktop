import { listProfiles, readActiveAccount } from "./account-store.ts";
import { bindingFor, readBindings } from "./bindings.ts";
import { formatFeedQuota, type CodexUsageSnapshot } from "./codex-usage.ts";
import { resolveLocale } from "./i18n.ts";

const POLL_MS = 2_000;
const WIDGET_KEY = "osdy-gentle:codex-accounts";
// Gentle's model profile names (isValidProfileName): a hand-edited binding
// that isn't one (an email, a path) never reaches the desktop feed.
const GENTLE_PROFILE_NAME = /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/;

function displayBinding(profile: string | undefined): string | null {
	return profile !== undefined && GENTLE_PROFILE_NAME.test(profile) && profile !== "constructor" && profile !== "prototype"
		? profile
		: null;
}

const EVENTS = {
	START: "session_start",
	SHUTDOWN: "session_shutdown",
	TREE: "session_tree",
	COMPACT: "session_compact",
} as const;
type WidgetEvent = (typeof EVENTS)[keyof typeof EVENTS];

interface WidgetUI {
	setWidget(key: string, lines: string[] | undefined): void;
}

interface WidgetContext {
	mode: string;
	ui: WidgetUI;
}

export interface WidgetHost {
	on?(event: WidgetEvent, handler: (event: unknown, ctx: WidgetContext) => void | Promise<void>): () => void;
}

interface WidgetSession {
	agentDir: string;
	ui: WidgetUI;
	json?: string;
}

function publish(session: WidgetSession, json: string | undefined): void {
	if (json === session.json) return;
	session.ui.setWidget(WIDGET_KEY, json === undefined ? undefined : [json]);
	session.json = json;
}

export function registerProfileWidget(
	host: WidgetHost,
	getAgentDir: () => string,
	readAccount = readActiveAccount,
	knownQuota: (agentDir: string, account: string) => CodexUsageSnapshot | undefined = () => undefined,
): () => Promise<void> | undefined {
	let session: WidgetSession | undefined;
	let timer: ReturnType<typeof setInterval> | undefined;
	let pending: Promise<void> | undefined;
	let revision = 0;

	async function readLatest(): Promise<void> {
		try {
			while (session) {
				const current = session;
				const requested = revision;
				const [names, account, bindings] = await Promise.all([
					listProfiles(current.agentDir).catch(() => []),
					readAccount(current.agentDir).catch(() => undefined),
					readBindings(current.agentDir).catch(() => ({})),
				]);
				// A boundary or replacement requested a newer read while this one was in flight.
				if (current !== session || requested !== revision) continue;
				const active = account?.status === "valid" && names.includes(account.profile) ? account.profile : null;
				const locale = resolveLocale();
				publish(current, JSON.stringify({
					v: 1,
					active,
					accounts: names.map((name) => {
						const quota = knownQuota(current.agentDir, name);
						return {
							name,
							active: name === active,
							binding: displayBinding(bindingFor(bindings, name)),
							quota: quota ? formatFeedQuota(quota, locale) : null,
						};
					}),
				}));
				return;
			}
		} finally {
			pending = undefined;
		}
	}

	function refresh(): Promise<void> | undefined {
		if (!session) return;
		revision++;
		return (pending ??= Promise.resolve().then(readLatest));
	}

	function stop(): void {
		if (timer !== undefined) clearInterval(timer);
		timer = undefined;
		const previous = session;
		session = undefined;
		if (previous) publish(previous, undefined);
	}

	// Pi emits session_start again for reload, new, resume, and fork. Tree and
	// compaction boundaries do not replace the extension runtime.
	host.on?.(EVENTS.START, (_event, ctx) => {
		stop();
		if (ctx.mode !== "rpc") return;
		session = { agentDir: getAgentDir(), ui: ctx.ui };
		const current = session;
		timer = setInterval(() => {
			if (session === current) return (pending ?? refresh())?.catch(() => {});
		}, POLL_MS);
		timer.unref();
		return refresh();
	});
	host.on?.(EVENTS.TREE, refresh);
	host.on?.(EVENTS.COMPACT, refresh);
	host.on?.(EVENTS.SHUTDOWN, stop);
	return refresh;
}
