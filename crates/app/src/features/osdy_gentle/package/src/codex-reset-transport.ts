import { arch, platform, release } from "node:os";
import type { CodexUsageAuth } from "./codex-usage.ts";

const DETAILS_URL = "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits";
const MAX_RESPONSE_BYTES = 1_000_000;
const DEFAULT_TIMEOUT_MS = 10_000;
const UNAVAILABLE = "Codex banked resets are unavailable";
const RESULT = { NOT_SENT: "not-sent", UNKNOWN: "unknown", CONFIRMED: "confirmed" } as const;
const CODE = { RESET: "reset", NOTHING: "nothing_to_reset", NO_CREDIT: "no_credit", REDEEMED: "already_redeemed" } as const;

type ResetCode = (typeof CODE)[keyof typeof CODE];
export interface CodexResetOutcome { code: ResetCode; windowsReset: number }
export interface CodexResetCredit { id: string; expiresAt: number }
export interface CodexResetRequest { requestId: string; creditId: string }
export interface CodexResetTransportOptions {
	fetch?: typeof globalThis.fetch;
	signal?: AbortSignal;
	timeoutMs?: number;
}
export type CodexResetResponse =
	| { kind: typeof RESULT.NOT_SENT }
	| { kind: typeof RESULT.UNKNOWN }
	| ({ kind: typeof RESULT.CONFIRMED } & CodexResetOutcome);

function object(value: unknown): Record<string, unknown> | undefined {
	return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : undefined;
}

export function isResetOutcome(code: unknown, windowsReset: unknown): boolean {
	return Object.values(CODE).some(value => value === code) &&
		typeof windowsReset === "number" && Number.isSafeInteger(windowsReset) && windowsReset >= 0;
}

export function isResetRequestId(value: unknown): value is string {
	return typeof value === "string" && /^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(value);
}

export function isResetCreditId(value: unknown): value is string {
	return typeof value === "string" && value.length <= 512 && !!value.trim() && !/[\x00-\x1f\x7f]/.test(value);
}

function expiryMillis(value: unknown): number | undefined {
	if (typeof value !== "string") return undefined;
	const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.\d+)?(?:Z|[+-](\d{2}):(\d{2}))$/.exec(value);
	if (!match) return undefined;
	const [, year, month, day, hour, minute, second, offsetHour = "0", offsetMinute = "0"] = match;
	const y = Number(year);
	const leap = y % 4 === 0 && (y % 100 !== 0 || y % 400 === 0);
	const days = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
	if (Number(day) < 1 || Number(day) > (days[Number(month) - 1] ?? 0) || Number(hour) > 23 ||
		Number(minute) > 59 || Number(second) > 59 || Number(offsetHour) > 23 || Number(offsetMinute) > 59) return undefined;
	const parsed = Date.parse(value);
	return Number.isSafeInteger(parsed) && parsed >= 0 ? parsed : undefined;
}

async function withAbort<T>(pending: Promise<T>, signal: AbortSignal): Promise<T> {
	let onAbort = () => {};
	const aborted = new Promise<never>((_resolve, reject) => {
		onAbort = () => reject(new Error(UNAVAILABLE));
		signal.addEventListener("abort", onAbort, { once: true });
		if (signal.aborted) onAbort();
	});
	try { return await Promise.race([pending, aborted]); }
	finally { signal.removeEventListener("abort", onAbort); }
}

async function boundedJson(response: Response, signal: AbortSignal): Promise<unknown> {
	const length = response.headers.get("content-length");
	if (!response.ok || signal.aborted || !response.body ||
		(length !== null && (!/^\d+$/.test(length) || Number(length) > MAX_RESPONSE_BYTES))) {
		void response.body?.cancel().catch(() => {});
		throw new Error(UNAVAILABLE);
	}
	const reader = response.body.getReader();
	const decoder = new TextDecoder("utf-8", { fatal: true });
	let size = 0;
	let text = "";
	try {
		for (;;) {
			const next = await withAbort(reader.read(), signal);
			if (next.done) break;
			size += next.value.byteLength;
			if (size > MAX_RESPONSE_BYTES) throw new Error(UNAVAILABLE);
			text += decoder.decode(next.value, { stream: true });
		}
		text += decoder.decode();
		return JSON.parse(text) as unknown;
	} catch (error) {
		void reader.cancel().catch(() => {});
		throw error;
	} finally { reader.releaseLock(); }
}

function requestInit(auth: CodexUsageAuth, options: CodexResetTransportOptions, request?: CodexResetRequest): RequestInit & { signal: AbortSignal } {
	const timeoutMs = options.timeoutMs ?? DEFAULT_TIMEOUT_MS;
	if (typeof auth?.accessToken !== "string" || !/^[\x21-\x7e]+$/.test(auth.accessToken) ||
		typeof auth.accountId !== "string" || !/^[\x21-\x7e]+$/.test(auth.accountId) ||
		!Number.isInteger(timeoutMs) || timeoutMs <= 0 || timeoutMs > 2_147_483_647) throw new Error(UNAVAILABLE);
	const timeout = AbortSignal.timeout(timeoutMs);
	const signal = options.signal ? AbortSignal.any([options.signal, timeout]) : timeout;
	if (signal.aborted) throw new Error(UNAVAILABLE);
	const headers = new Headers({
		accept: "application/json",
		authorization: `Bearer ${auth.accessToken}`,
		"chatgpt-account-id": auth.accountId,
		originator: "pi",
		"user-agent": `pi (${platform()} ${release()}; ${arch()})`,
	});
	const init = { headers, redirect: "error" as const, signal };
	if (!request) return { ...init, method: "GET" };
	headers.set("content-type", "application/json");
	return { ...init, method: "POST", body: JSON.stringify({ redeem_request_id: request.requestId, credit_id: request.creditId }) };
}

/** Read the account-bound detail list, not a quota estimate. Unknown expiry is ineligible. */
export async function listCodexResetCredits(auth: CodexUsageAuth, options: CodexResetTransportOptions = {}): Promise<CodexResetCredit[]> {
	try {
		const init = requestInit(auth, options);
		const response = await withAbort((options.fetch ?? globalThis.fetch)(DETAILS_URL, init), init.signal);
		const credits = object(await boundedJson(response, init.signal))?.credits;
		if (init.signal.aborted || !Array.isArray(credits)) throw new Error(UNAVAILABLE);
		const found = new Map<string, CodexResetCredit>();
		const ambiguous = new Set<string>();
		for (const entry of credits as unknown[]) {
			const credit = object(entry);
			if (!credit || credit.status !== "available" || credit.reset_type !== "codex_rate_limits" || !isResetCreditId(credit.id)) continue;
			const expiresAt = expiryMillis(credit.expires_at);
			const previous = found.get(credit.id);
			if (expiresAt === undefined || (previous && expiresAt !== previous.expiresAt)) ambiguous.add(credit.id);
			else if (!previous) found.set(credit.id, { id: credit.id, expiresAt });
		}
		return [...found.values()].filter(credit => !ambiguous.has(credit.id));
	} catch { throw new Error(UNAVAILABLE); }
}

/** Calling this starts the POST synchronously; response processing may outlive the switch fence. */
export async function consumeCodexReset(auth: CodexUsageAuth, request: CodexResetRequest, options: CodexResetTransportOptions = {}): Promise<CodexResetResponse> {
	let init: ReturnType<typeof requestInit>;
	try {
		if (!isResetRequestId(request?.requestId) || !isResetCreditId(request.creditId)) return { kind: RESULT.NOT_SENT };
		init = requestInit(auth, options, request);
	} catch { return { kind: RESULT.NOT_SENT }; }
	if (init.signal.aborted) return { kind: RESULT.NOT_SENT };
	try {
		const response = await withAbort((options.fetch ?? globalThis.fetch)(`${DETAILS_URL}/consume`, init), init.signal);
		const payload = object(await boundedJson(response, init.signal));
		const code = payload?.code;
		const windowsReset = payload?.windows_reset === undefined ? 0 : payload.windows_reset;
		if (!init.signal.aborted && isResetOutcome(code, windowsReset)) {
			return { kind: RESULT.CONFIRMED, code: code as ResetCode, windowsReset: windowsReset as number };
		}
	} catch {
		// Anything post-dispatch is uncertain unless the provider affirmatively reported an outcome.
	}
	return { kind: RESULT.UNKNOWN };
}
