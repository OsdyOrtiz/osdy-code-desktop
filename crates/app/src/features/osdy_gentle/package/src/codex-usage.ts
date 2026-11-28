/**
 * Codex subscription quota, trimmed port of Osdy-Pi's `codex-usage.ts` and
 * `profile-codex-usage.ts`. Read-only: never refreshes tokens, never falls back
 * to another account, never surfaces raw errors or bodies.
 */
import { resolveLocale, t, translator, type Translate } from "./i18n.ts";
import { arch, platform, release } from "node:os";

export type CodexUsageWindow = {
	usedPercent: number;
	windowMinutes: number | undefined;
	resetsAt: number | undefined;
};

export type CodexUsageSnapshot = {
	planType: string | undefined;
	primary: CodexUsageWindow | undefined;
	secondary: CodexUsageWindow | undefined;
	bankedResetCount: number | undefined;
	fetchedAt: number;
};

export type CodexUsageAuth = { accessToken: string; accountId: string };

export type FetchOptions = {
	fetch?: typeof globalThis.fetch;
	now?: () => number;
	timeoutMs?: number;
	signal?: AbortSignal;
};

const USAGE_URL = "https://chatgpt.com/backend-api/wham/usage";
const MAX_RESPONSE_BYTES = 1_000_000;
const DEFAULT_TIMEOUT_MS = 10_000;

type Rec = Record<string, unknown>;

function asRecord(value: unknown, label: string): Rec {
	if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error(t("Codex usage {label} is invalid", { label }));
	return value as Rec;
}

function parseWindow(value: unknown, label: string): CodexUsageWindow | undefined {
	if (value === undefined || value === null) return undefined;
	const w = asRecord(value, label);
	const used = w.used_percent;
	if (typeof used !== "number" || !Number.isFinite(used) || used < 0 || used > 100)
		throw new Error(t("Codex usage used_percent is invalid"));
	const seconds = w.limit_window_seconds;
	const resetAt = w.reset_at;
	return {
		usedPercent: used,
		windowMinutes: typeof seconds === "number" && Number.isFinite(seconds) && seconds >= 0 ? Math.round(seconds / 60) : undefined,
		resetsAt: typeof resetAt === "number" && Number.isFinite(resetAt) ? resetAt : undefined,
	};
}

export function parseCodexUsagePayload(input: unknown, fetchedAt = Date.now()): CodexUsageSnapshot {
	const payload = asRecord(input, "payload");
	const rate = asRecord(payload.rate_limit, "rate_limit");
	const primary = parseWindow(rate.primary_window, "primary_window");
	const secondary = parseWindow(rate.secondary_window, "secondary_window");
	if (!primary && !secondary) throw new Error(t("Codex usage does not contain any quota windows"));
	let bankedResetCount: number | undefined;
	const banked = payload.rate_limit_reset_credits;
	if (banked && typeof banked === "object" && !Array.isArray(banked)) {
		const count = (banked as Rec).available_count;
		if (typeof count === "number" && Number.isInteger(count) && count >= 0) bankedResetCount = count;
	}
	return {
		planType: typeof payload.plan_type === "string" ? payload.plan_type : undefined,
		primary,
		secondary,
		bankedResetCount,
		fetchedAt,
	};
}

function decodeJwtPart(part: string | undefined): Rec {
	if (!part || !/^[A-Za-z0-9_-]+$/.test(part)) throw new Error(t("Invalid token."));
	return asRecord(JSON.parse(Buffer.from(part, "base64url").toString("utf8")), "token");
}

export function extractCodexAccountId(accessToken: string): string {
	try {
		const claims = decodeJwtPart(accessToken.split(".")[1]);
		const id = asRecord(claims["https://api.openai.com/auth"], "token auth").chatgpt_account_id;
		if (typeof id !== "string" || !/^[A-Za-z0-9_-]+$/.test(id)) throw new Error("x");
		return id;
	} catch {
		throw new Error(t("Codex account ID is unavailable"));
	}
}

/** Unverified expiry claims: reject tokens that are clearly expired before sending them. */
export function assertTokenUsable(accessToken: string, now: number): void {
	const parts = accessToken.split(".");
	if (parts.length !== 3) throw new Error(t("Invalid token."));
	decodeJwtPart(parts[0]);
	const claims = decodeJwtPart(parts[1]);
	if ("exp" in claims && (typeof claims.exp !== "number" || claims.exp * 1000 <= now)) throw new Error(t("Expired token."));
}

async function readBoundedJson(response: Response): Promise<unknown> {
	const length = response.headers.get("content-length");
	if (length !== null && (!/^\d+$/.test(length) || Number(length) > MAX_RESPONSE_BYTES))
		throw new Error(t("Codex usage response is too large"));
	const text = await response.text();
	if (text.length > MAX_RESPONSE_BYTES) throw new Error(t("Codex usage response is too large"));
	try {
		return JSON.parse(text);
	} catch {
		throw new Error(t("Codex usage response is invalid"));
	}
}

export async function fetchCodexUsage(auth: CodexUsageAuth, options: FetchOptions = {}): Promise<CodexUsageSnapshot> {
	const fetcher = options.fetch ?? globalThis.fetch;
	const timeout = AbortSignal.timeout(options.timeoutMs ?? DEFAULT_TIMEOUT_MS);
	const signal = options.signal ? AbortSignal.any([options.signal, timeout]) : timeout;
	let response: Response;
	try {
		response = await fetcher(USAGE_URL, {
			method: "GET",
			headers: {
				accept: "application/json",
				authorization: `Bearer ${auth.accessToken}`,
				"chatgpt-account-id": auth.accountId,
				originator: "pi",
				"user-agent": `pi (${platform()} ${release()}; ${arch()})`,
			},
			redirect: "error",
			signal,
		});
	} catch {
		throw new Error(t("Codex usage is temporarily unavailable"));
	}
	if (response.status === 401) throw new Error(t("Codex session expired; sign in again"));
	if (response.status === 403) throw new Error(t("Codex usage access is unavailable or forbidden"));
	if (!response.ok) throw new Error(t("Codex usage is temporarily unavailable"));
	return parseCodexUsagePayload(await readBoundedJson(response), (options.now ?? Date.now)());
}

export type StoredCredential = { profile: string; access: string; expires: number; accountId?: string };

export type ProfileUsageResult =
	| { status: "ready"; profile: string; snapshot: CodexUsageSnapshot }
	| { status: "unavailable"; profile: string; reason: "stored-credentials-unavailable" | "remote-usage-unavailable" };

/** Preview a stored account's quota with its own saved token only. */
export async function requestProfileCodexUsage(
	profile: string,
	readCredential: (profile: string) => Promise<StoredCredential>,
	options: FetchOptions = {},
): Promise<ProfileUsageResult> {
	let auth: CodexUsageAuth;
	try {
		const credential = await readCredential(profile);
		const now = (options.now ?? Date.now)();
		if (credential.expires <= now) throw new Error("expired");
		assertTokenUsable(credential.access, now);
		const accountId = extractCodexAccountId(credential.access);
		if (credential.accountId !== undefined && credential.accountId !== accountId) throw new Error("mismatch");
		auth = { accessToken: credential.access, accountId };
	} catch {
		return { status: "unavailable", profile, reason: "stored-credentials-unavailable" };
	}
	try {
		return { status: "ready", profile, snapshot: await fetchCodexUsage(auth, options) };
	} catch {
		return { status: "unavailable", profile, reason: "remote-usage-unavailable" };
	}
}

function windowName(window: CodexUsageWindow, fallback: "session" | "weekly", t: Translate): string {
	const m = window.windowMinutes;
	if (m === undefined) return t(fallback);
	if (m >= 1440 && m % 1440 === 0) return m === 10080 ? t("weekly") : t("{days}d", { days: m / 1440 });
	if (m >= 60 && m % 60 === 0) return t("{hours}h", { hours: m / 60 });
	return t("{minutes}m", { minutes: m });
}

function left(window: CodexUsageWindow, t: Translate): string {
	return t("{percent}% left", { percent: Math.max(0, Math.min(100, Math.round(100 - window.usedPercent))) });
}

/** A display-only quota summary; never include arbitrary strings from the service. */
export function formatQuotaWindows({ primary, secondary }: CodexUsageSnapshot, locale = resolveLocale()): string {
	const t = translator(locale);
	const parts: string[] = [];
	if (primary) parts.push(`${windowName(primary, "session", t)} ${left(primary, t)}`);
	if (secondary) parts.push(`${windowName(secondary, "weekly", t)} ${left(secondary, t)}`);
	return parts.join(" · ");
}

/** Feed labels fit 64 chars even with two windows: safe-integer durations
 * have at most 16 digits, and an oversized duration keeps its percentage. */
export function formatFeedQuota(snapshot: CodexUsageSnapshot, locale = resolveLocale()): string {
	const bounded = (window: CodexUsageWindow | undefined) =>
		window?.windowMinutes !== undefined && !Number.isSafeInteger(window.windowMinutes)
			? { ...window, windowMinutes: undefined }
			: window;
	return formatQuotaWindows({ ...snapshot, primary: bounded(snapshot.primary), secondary: bounded(snapshot.secondary) }, locale);
}

/** One plain-text line for select labels and notifications. */
export function formatUsageLine(result: ProfileUsageResult, locale = resolveLocale()): string {
	const t = translator(locale);
	if (result.status !== "ready")
		return result.reason === "stored-credentials-unavailable" ? t("quota: no valid stored token") : t("quota: unavailable");
	const { planType, bankedResetCount } = result.snapshot;
	const windows = formatQuotaWindows(result.snapshot, locale);
	const parts = windows ? [windows] : [];
	if (bankedResetCount) parts.push(t(bankedResetCount === 1 ? "{count} banked reset" : "{count} banked resets", { count: bankedResetCount }));
	if (planType) parts.push(planType);
	return parts.join(" · ");
}
