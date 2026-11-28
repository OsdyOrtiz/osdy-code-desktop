import { test, type TestContext } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createCodexResetActions, type CodexResetActiveAuth } from "../src/codex-reset.ts";

const NOW = Date.parse("2035-01-01T00:00:00Z");
const expiry = "2035-01-02T00:00:00Z";
const creditId = " opaque/+é ";
const b64 = (value: unknown) => Buffer.from(JSON.stringify(value)).toString("base64url");
const auth = (accountId: string, version = "old", expires = NOW + 60_000) => ({
	accessToken: `${b64({ alg: "none" })}.${b64({ exp: expires / 1000, "https://api.openai.com/auth": { chatgpt_account_id: accountId } })}.${version}`,
	accountId,
	expires,
});
const unknownMessage = "Outcome unknown; do not start another reset. Check pending reset only reads the local journal and cannot verify this attempt with Codex.";

async function setup(t: TestContext) {
	const agentDir = await mkdtemp(join(tmpdir(), "osdy-reset-action-"));
	t.after(() => rm(agentDir, { recursive: true, force: true }));
	const calls: { url: string; init: RequestInit }[] = [];
	const dialogs: { title: string; choices: string[] }[] = [];
	const notices: [string, string | undefined][] = [];
	const f = {
		active: auth("acct-a"),
		now: NOW,
		select: (choices: string[]) => choices.at(-1),
		inFence: false,
		inOwnerLock: false,
		onFence: () => {},
		beforeOwnerSnapshot: (): void | Promise<void> => {},
		afterOwnerDispatch: () => {},
		freshExpiry: expiry,
		redeem: (): Response | Promise<Response> => Response.json({ code: "reset", windows_reset: 2 }),
	};
	const deps = {
		agentDir,
		now: () => f.now,
		readActiveAuth: () => f.active,
		async withCurrentAuthForDispatch(dispatch: (active: CodexResetActiveAuth) => void) {
			assert.equal(f.inFence, true);
			await f.beforeOwnerSnapshot();
			f.inOwnerLock = true;
			try { dispatch(f.active); f.afterOwnerDispatch(); } finally { f.inOwnerLock = false; }
			return true;
		},
		async withAccountSwitchFence<T>(run: () => Promise<T>) {
			assert.equal(f.inFence, false);
			f.inFence = true;
			try { f.onFence(); return await run(); } finally { f.inFence = false; }
		},
		fetch: (async (url, init = {}) => {
			calls.push({ url: String(url), init });
			if (init.method === "POST") {
				assert.equal(f.inFence, true);
				return f.redeem();
			}
			const expires_at = calls.length === 1 ? expiry : f.freshExpiry;
			return Response.json({ credits: ["other-opaque", creditId].map(id => ({ id, status: "available", reset_type: "codex_rate_limits", expires_at })) });
		}) as typeof fetch,
		ui: {
			async select(title: string, choices: string[]) {
				assert.equal(f.inFence, false);
				dialogs.push({ title, choices });
				return f.select(choices);
			},
			notify(message: string, level?: string) { notices.push([message, level]); },
		},
	};
	return { f, deps, calls, dialogs, notices, actions: createCodexResetActions(deps) };
}

test("cancel first: no attempt or credit spend and the journal check does not fetch", async t => {
	const { f, calls, dialogs, notices, actions } = await setup(t);
	f.select = choices => choices[0];
	assert.equal(await actions.redeem(), "cancelled");
	assert.deepEqual(dialogs[1]?.choices, ["Cancel", "Use reset"]);
	assert.equal(await actions.check(), "none");
	assert.deepEqual(calls.map(c => c.init.method), ["GET"]);
	assert.deepEqual(notices, [
		["Banked reset action cancelled; no new reset was sent.", "info"],
		["No banked reset attempt is saved for the active Codex account.", "info"],
	]);
});

test("POSTs exactly the selected opaque credit once using the newest verified active token, after saving pending", async t => {
	const { f, deps, calls, dialogs, notices, actions } = await setup(t);
	f.select = choices => {
		if (choices[0] === "Cancel") f.active = auth("acct-a", "fresh");
		return choices.at(-1);
	};
	let inFlightCheck: ReturnType<typeof actions.check> | undefined;
	f.redeem = async () => {
		assert.equal(f.inOwnerLock, true);
		inFlightCheck = createCodexResetActions(deps).check();
		await inFlightCheck;
		assert.equal(f.inFence, false);
		assert.equal(f.inOwnerLock, false);
		return Response.json({ code: "reset", windows_reset: 2 });
	};
	assert.equal(await actions.redeem(), "confirmed");
	assert.equal(await inFlightCheck, "pending");
	assert.equal(await createCodexResetActions(deps).check(), "confirmed");
	assert.deepEqual(calls.map(c => c.init.method), ["GET", "GET", "POST"]);
	const post = calls[2]!;
	const body = JSON.parse(String(post.init.body));
	assert.equal(post.url, "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits/consume");
	assert.equal(post.init.redirect, "error");
	assert.equal(new Headers(post.init.headers).get("authorization"), `Bearer ${f.active.accessToken}`);
	assert.equal(new Headers(calls[1]?.init.headers).get("authorization"), `Bearer ${f.active.accessToken}`);
	assert.match(body.redeem_request_id, /^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/);
	assert.deepEqual(body, { redeem_request_id: body.redeem_request_id, credit_id: creditId });
	assert.deepEqual(notices, [[unknownMessage, "warning"], ["Banked reset consumed; 2 rate-limit windows reset.", "info"], ["Banked reset consumed; 2 rate-limit windows reset.", "info"]]);
	for (const secret of [creditId, "other-opaque", f.active.accessToken, body.redeem_request_id]) {
		assert.equal(JSON.stringify([dialogs, notices]).includes(secret), false);
	}
});

test("unknown survives action recreation, blocks any other POST for the actual account, and never triggers quota GET", async t => {
	const { f, deps, calls, notices, actions } = await setup(t);
	f.redeem = () => { throw new Error(`private ${creditId}`); };
	assert.equal(await actions.redeem(), "pending");
	f.active = auth("acct-a", "expired", NOW - 1);
	const restarted = createCodexResetActions(deps);
	assert.equal(await restarted.check(), "pending");
	f.active = auth("acct-a", "rotated");
	assert.equal(await restarted.redeem(), "pending");
	f.active = auth("acct-b");
	assert.equal(await restarted.check(), "none");
	assert.deepEqual(calls.map(c => c.init.method), ["GET", "GET", "POST"]);
	assert.deepEqual(notices, [
		[unknownMessage, "warning"], [unknownMessage, "warning"], [unknownMessage, "warning"],
		["No banked reset attempt is saved for the active Codex account.", "info"],
	]);
});

test("fresh details must match confirmation; missing, expired, or changed expiry never creates an attempt", async t => {
	for (const changed of ["", "2035-01-01T00:00:00Z", "2035-01-03T00:00:00Z"]) {
		const { f, calls, notices, actions } = await setup(t);
		f.freshExpiry = changed;
		assert.equal(await actions.redeem(), "unavailable");
		assert.equal(await actions.check(), "none");
		assert.deepEqual(calls.map(c => c.init.method), ["GET", "GET"]);
		assert.deepEqual(notices, [
			["No currently available banked reset could be selected. Refresh usage to check availability.", "warning"],
			["No banked reset attempt is saved for the active Codex account.", "info"],
		]);
	}
});

test("a native credential change immediately before the owner snapshot cancels a journaled dispatch", async t => {
	for (const changed of [auth("acct-b"), auth("acct-a", "rotated")]) {
		const { f, deps, calls, notices, actions } = await setup(t);
		f.beforeOwnerSnapshot = async () => {
			assert.equal(await createCodexResetActions(deps).check(), "pending");
			f.active = changed;
		};
		assert.equal(await actions.redeem(), "cancelled");
		f.active = auth("acct-a");
		assert.equal(await actions.check(), "none");
		assert.deepEqual(calls.map(c => c.init.method), ["GET", "GET"]);
		assert.deepEqual(notices, [
			[unknownMessage, "warning"],
			["Banked reset action cancelled; no new reset was sent.", "info"],
			["No banked reset attempt is saved for the active Codex account.", "info"],
		]);
	}
});

test("credit expiry at the owner lock boundary prevents the POST", async t => {
	const { f, calls, actions, notices } = await setup(t);
	f.active = auth("acct-a", "old", NOW + 2 * 86_400_000);
	f.beforeOwnerSnapshot = () => { f.now = Date.parse(expiry); };
	assert.equal(await actions.redeem(), "cancelled");
	assert.equal(await actions.check(), "none");
	assert.deepEqual(calls.map(c => c.init.method), ["GET", "GET"]);
	assert.deepEqual(notices, [
		["Banked reset action cancelled; no new reset was sent.", "info"],
		["No banked reset attempt is saved for the active Codex account.", "info"],
	]);
});

test("an aborted POST with no response survives owner failure and cannot resubmit", { timeout: 3_000 }, async t => {
	const { f, deps, calls, notices } = await setup(t);
	const posted = Promise.withResolvers<void>();
	const controller = new AbortController();
	const actions = createCodexResetActions({ ...deps, signal: controller.signal });
	f.redeem = () => { posted.resolve(); return new Promise<Response>(() => {}); };
	f.afterOwnerDispatch = () => { throw new Error("synthetic owner lock failure after dispatch"); };
	const redeeming = actions.redeem();
	await posted.promise;
	assert.equal(f.inOwnerLock, false);
	const restarted = createCodexResetActions(deps);
	assert.equal(await restarted.check(), "pending");
	assert.equal(await restarted.redeem(), "pending");
	controller.abort();
	assert.equal(await redeeming, "pending");
	assert.equal(await restarted.redeem(), "pending");
	assert.deepEqual(calls.map(c => c.init.method), ["GET", "GET", "POST"]);
	assert.equal(calls[2]?.init.signal?.aborted, true);
	assert.deepEqual(notices, Array.from({ length: 4 }, () => [unknownMessage, "warning"]));
});

test("a switch before the fenced final read cannot create an attempt or send a POST", async t => {
	const { f, calls, notices, actions } = await setup(t);
	f.onFence = () => { f.active = auth("acct-b"); };
	assert.equal(await actions.redeem(), "cancelled");
	f.active = auth("acct-a");
	assert.equal(await actions.check(), "none");
	assert.deepEqual(calls.map(c => c.init.method), ["GET", "GET"]);
	assert.deepEqual(notices, [
		["Banked reset action cancelled; no new reset was sent.", "info"],
		["No banked reset attempt is saved for the active Codex account.", "info"],
	]);
});
