import { currentLocale, translator, type Locale, type Translate } from "./i18n.ts";
import { randomUUID } from "node:crypto";
import { assertTokenUsable, extractCodexAccountId, type CodexUsageAuth } from "./codex-usage.ts";
import { createCodexResetStore, type CodexResetAttempt, type CodexResetLease, type CodexResetStore } from "./codex-reset-store.ts";
import { consumeCodexReset, listCodexResetCredits, type CodexResetOutcome, type CodexResetResponse, type CodexResetTransportOptions } from "./codex-reset-transport.ts";

export interface CodexResetActiveAuth {
	accessToken: string;
	/** Optional expiry and account ID from the active runtime credential. */
	expires?: number;
	accountId?: string;
}
export interface CodexResetUI {
	select(title: string, options: string[]): Promise<string | undefined>;
	notify(message: string, level?: "info" | "warning"): void;
}
interface CodexResetDependencies extends CodexResetTransportOptions {
	/** Read the newest active runtime credential without refreshing or contacting the provider. Never a saved profile preview. */
	readActiveAuth(): CodexResetActiveAuth | undefined | Promise<CodexResetActiveAuth | undefined>;
	/** Invoke dispatch synchronously under the session owner's auth lock with its current OAuth snapshot. False means the boundary failed. */
	withCurrentAuthForDispatch(dispatch: (active: CodexResetActiveAuth) => void): Promise<boolean>;
	/** Use the same cross-process fence as account switching. This callback must not acquire a reset lease. */
	withAccountSwitchFence<T>(run: () => Promise<T>): Promise<T>;
	ui: CodexResetUI;
	now?: () => number;
}
export type CodexResetOptions = CodexResetDependencies & ({ agentDir: string; journal?: never } | { agentDir?: never; journal: CodexResetStore });

const STATUS = {
	NONE: "none", CANCELLED: "cancelled", BLOCKED: "blocked", UNAVAILABLE: "unavailable", PENDING: "pending", CONFIRMED: "confirmed",
} as const;
export type CodexResetActionStatus = (typeof STATUS)[keyof typeof STATUS];
export interface CodexResetActions {
	redeem(): Promise<CodexResetActionStatus>;
	check(): Promise<CodexResetActionStatus>;
}
type ActionResult =
	| { status: Exclude<CodexResetActionStatus, typeof STATUS.CONFIRMED>; journalFailed?: boolean }
	| { status: typeof STATUS.CONFIRMED; outcome: CodexResetOutcome; journalFailed?: boolean };

const MESSAGE = {
	[STATUS.NONE]: "No banked reset attempt is saved for the active Codex account.",
	[STATUS.CANCELLED]: "Banked reset action cancelled; no new reset was sent.",
	[STATUS.BLOCKED]: "Banked reset action blocked. The local attempt store or account switch fence is unavailable; do not retry automatically.",
	[STATUS.UNAVAILABLE]: "No currently available banked reset could be selected. Refresh usage to check availability.",
	[STATUS.PENDING]: "Outcome unknown; do not start another reset. Check pending reset only reads the local journal and cannot verify this attempt with Codex.",
} as const;

function outcomeMessage({ code, windowsReset }: CodexResetOutcome, t: Translate) {
	switch (code) {
		case "reset": return t("Banked reset consumed; {windowsReset} rate-limit windows reset.", { windowsReset });
		case "nothing_to_reset": return t("No rate limits needed resetting; a banked reset was not consumed.");
		case "no_credit": return t("No banked reset was available; a reset was not consumed.");
		case "already_redeemed": return t("Codex reported that this banked reset was already redeemed.");
	}
}

export function createCodexResetActions(deps: CodexResetOptions): CodexResetActions {
	const store = deps.agentDir !== undefined ? createCodexResetStore(deps.agentDir) : deps.journal;
	const now = deps.now ?? Date.now;

	function validateCredential(active: CodexResetActiveAuth | undefined, requireUsable: boolean): CodexUsageAuth | undefined {
		try {
			if (!active || deps.signal?.aborted) return undefined;
			const { accessToken, expires } = active;
			const accountId = extractCodexAccountId(accessToken);
			if (active.accountId !== undefined && active.accountId !== accountId) return undefined;
			if (requireUsable) {
				const time = now();
				if (!Number.isFinite(time) || (expires !== undefined && (!Number.isFinite(expires) || expires <= time)) ||
					!/^[\x21-\x7e]+$/.test(accessToken)) return undefined;
				assertTokenUsable(accessToken, time);
			}
			return { accessToken, accountId };
		} catch { return undefined; }
	}

	async function credential(requireUsable: boolean): Promise<CodexUsageAuth | undefined> {
		try {
			if (!deps.signal?.aborted) return validateCredential(await deps.readActiveAuth(), requireUsable);
		} catch { return undefined; }
	}

	function present(result: ActionResult, t: Translate): CodexResetActionStatus {
		let message = result.status === STATUS.CONFIRMED ? outcomeMessage(result.outcome, t) : t(MESSAGE[result.status]);
		if (result.journalFailed) message += t(" Local attempt bookkeeping failed; do not start another reset before manual verification.");
		const info = result.status === STATUS.CONFIRMED || result.status === STATUS.NONE || result.status === STATUS.CANCELLED;
		deps.ui.notify(message, info && !result.journalFailed ? "info" : "warning");
		return result.status;
	}

	async function check(): Promise<ActionResult> {
		const active = await credential(false); // Local identity is useful even if the token is expired.
		if (!active) return { status: STATUS.UNAVAILABLE };
		try {
			const saved = await store.read(active.accountId);
			if ((await credential(false))?.accountId !== active.accountId) return { status: STATUS.CANCELLED };
			if (saved?.status === "confirmed") return { status: STATUS.CONFIRMED, outcome: saved };
			return { status: saved ? STATUS.PENDING : STATUS.NONE };
		} catch { return { status: STATUS.BLOCKED }; }
	}

	async function redeem(locale: Locale, t: Translate): Promise<ActionResult> {
		const expiryLabel = (expiresAt: number) => t("expires {date} (local time)", { date: new Date(expiresAt).toLocaleString(locale) });
		const active = await credential(false);
		if (!active || deps.signal?.aborted) return { status: active ? STATUS.CANCELLED : STATUS.UNAVAILABLE };
		const accountId = active.accountId;
		let lease: CodexResetLease;
		try {
			if ((await store.read(accountId))?.status === "pending") return { status: STATUS.PENDING };
			lease = await store.acquire(accountId);
		} catch { return { status: STATUS.BLOCKED }; }

		async function attempt(): Promise<ActionResult> {
			if ((await lease.read())?.status === "pending") return { status: STATUS.PENDING };
			const initial = await credential(true);
			if (deps.signal?.aborted) return { status: STATUS.CANCELLED };
			if (!initial) return { status: STATUS.UNAVAILABLE };
			if (initial.accountId !== accountId) return { status: STATUS.CANCELLED };
			try {
				const credits = (await listCodexResetCredits(initial, deps)).filter(credit => credit.expiresAt > now());
				if (deps.signal?.aborted || (await credential(false))?.accountId !== initial.accountId) return { status: STATUS.CANCELLED };
				if (!credits.length) return { status: STATUS.UNAVAILABLE };
				const labels = credits.map((credit, index) => t("Reset {number} — {expiry}", { number: index + 1, expiry: expiryLabel(credit.expiresAt) }));
				const label = await deps.ui.select(t("Choose a banked reset for the active Codex account"), [...labels]);
				const selected = label === undefined ? undefined : credits[labels.indexOf(label)];
				if (!selected || deps.signal?.aborted || (await credential(false))?.accountId !== initial.accountId) return { status: STATUS.CANCELLED };
				const accept = t("Use reset");
				const confirmation = await deps.ui.select(
					t("For the active Codex account: Use one banked reset; one banked reset will be consumed if rate limits are reset.\nReset {expiry}.", { expiry: expiryLabel(selected.expiresAt) }),
					[t("Cancel"), accept],
				);
				if (confirmation !== accept || deps.signal?.aborted) return { status: STATUS.CANCELLED };
				const latest = await credential(true);
				if (latest?.accountId !== initial.accountId) return { status: STATUS.CANCELLED };
				const fresh = (await listCodexResetCredits(latest, deps)).find(credit => credit.id === selected.id);
				if (!fresh || fresh.expiresAt !== selected.expiresAt || !(fresh.expiresAt > now())) return { status: STATUS.UNAVAILABLE };
				return dispatch(lease, latest, { creditId: selected.id, requestId: randomUUID(), expiresAt: fresh.expiresAt });
			} catch { return { status: deps.signal?.aborted ? STATUS.CANCELLED : STATUS.UNAVAILABLE }; }
		}

		let result: ActionResult;
		try { result = await attempt(); }
		catch { result = { status: STATUS.BLOCKED }; }
		try { await lease.release(); }
		catch {
			if (result.status === STATUS.CONFIRMED || result.status === STATUS.PENDING) return { ...result, journalFailed: true };
			return { status: STATUS.BLOCKED, journalFailed: true };
		}
		return result;
	}

	async function dispatch(lease: CodexResetLease, freshAuth: CodexUsageAuth, attempt: CodexResetAttempt): Promise<ActionResult> {
		let saved = false;
		let response: Promise<CodexResetResponse> | undefined;
		let fenceFailed = false;
		const stillCurrent = (active: CodexUsageAuth | undefined) => active?.accountId === freshAuth.accountId &&
			active.accessToken === freshAuth.accessToken && !deps.signal?.aborted && attempt.expiresAt > now();
		try {
			// Lock order: account lease, switch fence, then Pi's auth lock. The POST response is awaited outside both fences.
			await deps.withAccountSwitchFence(async () => {
				if (!stillCurrent(await credential(true))) return;
				await lease.start(attempt);
				saved = true;
				let dispatchOpen = true;
				try {
					const locked = await deps.withCurrentAuthForDispatch(active => {
						if (!dispatchOpen || response) return;
						const current = validateCredential(active, true);
						if (current && stillCurrent(current)) response = consumeCodexReset(current, attempt, deps);
					});
					if (!locked && !deps.signal?.aborted) fenceFailed = true;
				} finally { dispatchOpen = false; }
			});
		} catch { fenceFailed = true; }
		const outcome = await response;
		if (!outcome || outcome.kind === "not-sent") {
			if (saved) {
				try { await lease.discard(attempt); }
				catch { return { status: STATUS.BLOCKED, journalFailed: true }; }
			}
			return { status: fenceFailed || (outcome && !deps.signal?.aborted) ? STATUS.BLOCKED : STATUS.CANCELLED };
		}
		if (outcome.kind === "unknown") return { status: STATUS.PENDING };
		try { await lease.confirm(attempt, outcome); }
		catch { return { status: STATUS.CONFIRMED, outcome, journalFailed: true }; }
		return { status: STATUS.CONFIRMED, outcome };
	}

	return {
		async redeem() {
			const locale = currentLocale();
			const t = translator(locale);
			return present(await redeem(locale, t), t);
		},
		async check() {
			const t = translator();
			return present(await check(), t);
		},
	};
}
