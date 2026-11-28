import { test } from "node:test";
import assert from "node:assert/strict";
import {
	extractCodexAccountId,
	formatUsageLine,
	parseCodexUsagePayload,
	requestProfileCodexUsage,
	type StoredCredential,
} from "../src/codex-usage.ts";

const b64 = (value: unknown) => Buffer.from(JSON.stringify(value)).toString("base64url");
const token = (claims: Record<string, unknown>) => `${b64({ alg: "none" })}.${b64(claims)}.sig`;
const NOW = 1_800_000_000_000;
const validToken = token({ exp: NOW / 1000 + 3600, "https://api.openai.com/auth": { chatgpt_account_id: "acct-1" } });

const payload = {
	plan_type: "pro",
	rate_limit: {
		primary_window: { used_percent: 28, limit_window_seconds: 18000, reset_at: 1 },
		secondary_window: { used_percent: 60.4, limit_window_seconds: 604800 },
	},
	rate_limit_reset_credits: { available_count: 2 },
};

function fakeFetch(status: number, body: unknown, seen: { url?: string; headers?: Record<string, string> } = {}) {
	return (async (url: string, init: RequestInit) => {
		seen.url = url;
		seen.headers = init.headers as Record<string, string>;
		return new Response(JSON.stringify(body), { status });
	}) as unknown as typeof fetch;
}

test("parses quota windows and banked resets", () => {
	const snap = parseCodexUsagePayload(payload, 5);
	assert.deepEqual(snap, {
		planType: "pro",
		primary: { usedPercent: 28, windowMinutes: 300, resetsAt: 1 },
		secondary: { usedPercent: 60.4, windowMinutes: 10080, resetsAt: undefined },
		bankedResetCount: 2,
		fetchedAt: 5,
	});
	assert.throws(() => parseCodexUsagePayload({ rate_limit: {} }), /any quota windows/);
});

test("extracts the ChatGPT account id from the token", () => {
	assert.equal(extractCodexAccountId(validToken), "acct-1");
	assert.throws(() => extractCodexAccountId("bad"), /unavailable/);
});

test("stored profile preview sends the profile's own token and formats one line", async () => {
	const seen: { url?: string; headers?: Record<string, string> } = {};
	const read = async (p: string): Promise<StoredCredential> => ({ profile: p, access: validToken, expires: NOW + 1 });
	const result = await requestProfileCodexUsage("Work", read, { fetch: fakeFetch(200, payload, seen), now: () => NOW });
	assert.equal(result.status, "ready");
	assert.equal(seen.url, "https://chatgpt.com/backend-api/wham/usage");
	assert.equal(seen.headers?.authorization, `Bearer ${validToken}`);
	assert.equal(seen.headers?.["chatgpt-account-id"], "acct-1");
	assert.equal(formatUsageLine(result), "5h 72% left · weekly 40% left · 2 banked resets · pro");
});

test("expired or mismatched stored tokens are never sent", async () => {
	let calls = 0;
	const counting = (async () => { calls++; return new Response("{}"); }) as unknown as typeof fetch;
	const expired = async (p: string): Promise<StoredCredential> => ({ profile: p, access: validToken, expires: NOW - 1 });
	const mismatch = async (p: string): Promise<StoredCredential> => ({ profile: p, access: validToken, expires: NOW + 1, accountId: "other" });
	for (const read of [expired, mismatch]) {
		const r = await requestProfileCodexUsage("Work", read, { fetch: counting, now: () => NOW });
		assert.deepEqual(r, { status: "unavailable", profile: "Work", reason: "stored-credentials-unavailable" });
	}
	assert.equal(calls, 0);
	assert.equal(formatUsageLine({ status: "unavailable", profile: "Work", reason: "stored-credentials-unavailable" }), "quota: no valid stored token");
});

test("remote failures are reported without raw errors", async () => {
	const read = async (p: string): Promise<StoredCredential> => ({ profile: p, access: validToken, expires: NOW + 1 });
	const r = await requestProfileCodexUsage("Work", read, { fetch: fakeFetch(401, { secret: "x" }), now: () => NOW });
	assert.deepEqual(r, { status: "unavailable", profile: "Work", reason: "remote-usage-unavailable" });
	assert.equal(formatUsageLine(r), "quota: unavailable");
});
