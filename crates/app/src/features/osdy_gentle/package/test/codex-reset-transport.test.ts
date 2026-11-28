import { test } from "node:test";
import assert from "node:assert/strict";
import { listCodexResetCredits, consumeCodexReset } from "../src/codex-reset-transport.ts";

const auth = { accountId: "mock-account", accessToken: "synthetic-token" };
const creditId = " exact/+é ";
const request = { creditId, requestId: "11111111-1111-4111-8111-111111111111" };
const expiry = "2035-01-02T00:00:00Z";

test("credit details GET on provider origin filters ineligible entries and preserves the exact ID", async () => {
	const available = { id: creditId, status: "available", reset_type: "codex_rate_limits", expires_at: expiry };
	const fetcher: typeof fetch = async (url, init) => {
		assert.equal(url, "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits");
		assert.equal(init?.method, "GET");
		assert.equal(init?.redirect, "error");
		assert.equal(new Headers(init?.headers).get("chatgpt-account-id"), auth.accountId);
		assert.equal(new Headers(init?.headers).get("authorization"), `Bearer ${auth.accessToken}`);
		return Response.json({ credits: [available, available, { ...available, id: "wrong-type", reset_type: "other" },
			{ ...available, id: "used", status: "redeemed" }, { ...available, id: "invalid-date", expires_at: "2035-02-30T00:00:00Z" }] });
	};
	assert.deepEqual(await listCodexResetCredits(auth, { fetch: fetcher }), [{ id: creditId, expiresAt: Date.parse(expiry) }]);
});

test("only recognized 2xx consume responses prove outcomes; no failure retries or raw error exposure", async () => {
	for (const body of [
		() => Response.json({ code: "reset", windows_reset: 2 }, { status: 409 }),
		() => Response.json({ code: "reset", windows_reset: -1 }),
		() => Response.json({ code: creditId }),
		() => new Response(creditId),
		() => { throw new Error(creditId); },
	]) {
		let calls = 0;
		const fetcher: typeof fetch = async () => { calls++; return body(); };
		assert.deepEqual(await consumeCodexReset(auth, request, { fetch: fetcher }), { kind: "unknown" });
		assert.equal(calls, 1);
	}
	for (const code of ["reset", "nothing_to_reset", "no_credit", "already_redeemed"]) {
		let calls = 0;
		const fetcher: typeof fetch = async () => { calls++; return Response.json({ code }); };
		assert.deepEqual(await consumeCodexReset(auth, request, { fetch: fetcher }), { kind: "confirmed", code, windowsReset: 0 });
		assert.equal(calls, 1);
	}
});

test("invalid or pre-aborted POST input never dispatches", async () => {
	let calls = 0;
	const fetcher: typeof fetch = async () => { calls++; return new Response(); };
	assert.deepEqual(await consumeCodexReset(auth, { ...request, requestId: "bad" }, { fetch: fetcher }), { kind: "not-sent" });
	assert.deepEqual(await consumeCodexReset(auth, request, { fetch: fetcher, signal: AbortSignal.abort() }), { kind: "not-sent" });
	assert.equal(calls, 0);
});
