import assert from "node:assert/strict";
import { mkdtemp, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, type TestContext } from "node:test";
import osdyGentle from "../src/index.ts";
import { createProfile, readActiveAccount } from "../src/account-store.ts";
import { readBindings, setBinding } from "../src/bindings.ts";

async function setup(t: TestContext) {
	const directory = await mkdtemp(join(tmpdir(), "osdy-usage-"));
	const previous = process.env.PI_CODING_AGENT_DIR;
	process.env.PI_CODING_AGENT_DIR = directory;
	t.after(async () => {
		if (previous === undefined) delete process.env.PI_CODING_AGENT_DIR;
		else process.env.PI_CODING_AGENT_DIR = previous;
		await rm(directory, { recursive: true, force: true });
	});
	await createProfile(directory, "Personal");
	await createProfile(directory, "Work");
	const store = join(directory, "osdy-gentle");
	await writeFile(join(store, "active-account.json"), JSON.stringify({ version: 1, profile: "Personal" }));
	await setBinding(directory, "Work", "model-work");
	const jwt = (value: object) => Buffer.from(JSON.stringify(value)).toString("base64url");
	const access = `${jwt({ alg: "none" })}.${jwt({ "https://api.openai.com/auth": { chatgpt_account_id: "fixture-work" } })}.fixture`;
	await writeFile(join(store, "profiles", "Work", "auth.json"), JSON.stringify({
		"openai-codex": { type: "oauth", access, expires: Date.now() + 60_000 },
	}), { mode: 0o600 });
	const requests: (string | null)[] = [];
	let fail = false;
	let primarySeconds = 18_000;
	let secondarySeconds = 604_800;
	t.mock.method(globalThis, "fetch", async (_url: string, init: RequestInit) => {
		requests.push(new Headers(init.headers).get("chatgpt-account-id"));
		if (fail) return new Response(null, { status: 503 });
		return Response.json({
			plan_type: "provider-supplied metadata",
			rate_limit_reset_credits: { available_count: 3 },
			rate_limit: {
				primary_window: { used_percent: 25, limit_window_seconds: primarySeconds },
				secondary_window: { used_percent: 60, limit_window_seconds: secondarySeconds },
			},
		});
	});
	type Host = Parameters<typeof osdyGentle>[0];
	type Subscribe = NonNullable<Host["on"]>;
	const commands = new Map<string, Parameters<Host["registerCommand"]>[1]>();
	const handlers = new Map<Parameters<Subscribe>[0], Parameters<Subscribe>[1][]>();
	const publications: unknown[] = [];
	const notices: unknown[] = [];
	const unexpected = () => assert.fail("usage must not prompt, wait for idle or emit a profile event");
	const ctx = { mode: "rpc", isIdle: unexpected, ui: {
		select: unexpected, confirm: unexpected, input: unexpected,
		notify: (...args: unknown[]) => notices.push(args),
		setWidget: (key: string, lines: string[] | undefined) => {
			assert.equal(key, "osdy-gentle:codex-accounts");
			if (lines) {
				assert.equal(lines.length, 1);
				publications.push(JSON.parse(lines[0]!));
			}
		},
	} };
	osdyGentle({
		on(event, handler) {
			handlers.set(event, [...(handlers.get(event) ?? []), handler]);
			return () => {};
		},
		events: { emit: unexpected },
		registerCommand(name, command) { commands.set(name, command); },
	});
	async function event(name: Parameters<Subscribe>[0]) {
		for (const handler of handlers.get(name) ?? []) await handler({}, ctx);
	}
	t.after(() => event("session_shutdown"));
	await event("session_start");
	const active = await readActiveAccount(directory);
	const bindings = await readBindings(directory);
	return {
		publications, notices, requests, event,
		async runPicker(name: string, select: (title: string, options: string[]) => Promise<string | undefined>) {
			await commands.get(name)!.handler("", { ...ctx, ui: { ...ctx.ui, select } });
		},
		fail: () => { fail = true; },
		largeWindows: () => { primarySeconds = 1e22; secondarySeconds = 1e24; },
		async run(args: string) {
			const command = commands.get("osdy-usage");
			assert.ok(command, "registers /osdy-usage");
			await command.handler(args, ctx);
			assert.deepEqual(await readActiveAccount(directory), active);
			assert.deepEqual(await readBindings(directory), bindings);
			await assert.rejects(stat(join(directory, "auth.json")), { code: "ENOENT" });
		},
	};
}

function feed(quota: string | null) {
	return { v: 1, active: "Personal", accounts: [
		{ name: "Personal", active: true, binding: null, quota: null },
		{ name: "Work", active: false, binding: "model-work", quota },
	] };
}

test("/osdy-usage fetches only the named account and immediately republishes both windows", async (t) => {
	const h = await setup(t);
	await h.run("  wOrK  ");
	assert.deepEqual(h.requests, ["fixture-work"]);
	assert.deepEqual(h.publications, [feed(null), feed("5h 75% left · weekly 40% left")]);
	assert.deepEqual(h.notices, []);
	await h.run("Work");
	assert.equal(h.publications.length, 2, "identical quota does not republish");
});

test("/osdy-usage rejects unknown and missing names without fetching or publishing", async (t) => {
	const h = await setup(t);
	await h.run("unknown account");
	await h.run("  ");
	assert.deepEqual(h.notices, [["Unknown account: unknown account", "error"], ["Usage: /osdy-usage <name>", "warning"]]);
	assert.deepEqual(h.requests, []);
	await h.event("session_tree");
	assert.deepEqual(h.publications, [feed(null)]);
});

test("/osdy-usage lookup failure keeps the previous quota", async (t) => {
	const h = await setup(t);
	await h.run("Work");
	h.fail();
	await h.run("Work");
	assert.deepEqual(h.requests, ["fixture-work", "fixture-work"]);
	assert.deepEqual(h.notices, [["Codex quota unavailable.", "warning"]]);
	await h.event("session_tree");
	assert.deepEqual(h.publications, [feed(null), feed("5h 75% left · weekly 40% left")]);
});

test("/osdy-usage keeps both windows in the 64-character budget for oversized durations", async (t) => {
	const h = await setup(t);
	h.largeWindows();
	await h.run("Work");
	const summary = "session 75% left · weekly 40% left";
	assert.ok(summary.length <= 64);
	assert.deepEqual(h.publications, [feed(null), feed(summary)]);
	assert.deepEqual(h.requests, ["fixture-work"]);
	assert.deepEqual(h.notices, []);
});

const legacyQuota = "5h 75% left · weekly 40% left · 3 banked resets · provider-supplied metadata";

test("/osdy-account still shows its quota notification and the feed refreshes on a session boundary", async (t) => {
	const h = await setup(t);
	await h.runPicker("osdy-account", async (title, options) => {
		if (title === "Osdy accounts") return "Check account quota";
		assert.equal(title, "Account quota");
		assert.deepEqual(options, ["Personal (active)", "Work — model: model-work"]);
		return options[1];
	});
	assert.deepEqual(h.notices, [[`Work: ${legacyQuota}`, "info"]]);
	assert.deepEqual(h.publications, [feed(null)]);
	await h.event("session_tree");
	assert.deepEqual(h.publications, [feed(null), feed("5h 75% left · weekly 40% left")]);
});

test("/osdy-profile still displays legacy quota details before selection", async (t) => {
	const h = await setup(t);
	await h.runPicker("osdy-profile", async (title, options) => {
		assert.equal(title, "Switch Osdy profile (account + model)");
		assert.deepEqual(options, ["Personal (active) — quota: no valid stored token", `Work — model: model-work — ${legacyQuota}`]);
		return undefined;
	});
	assert.deepEqual(h.notices, [["Checking Codex quota for stored accounts…", "info"]]);
});

test("/osdy-usage never lets an older lookup overwrite a newer one for the same account", async (t) => {
	const h = await setup(t);
	let release!: () => void;
	const gate = new Promise<void>((resolve) => { release = resolve; });
	let calls = 0;
	t.mock.method(globalThis, "fetch", async () => {
		const call = ++calls;
		if (call === 1) await gate; // the older lookup answers last
		return Response.json({ rate_limit: {
			primary_window: { used_percent: call === 1 ? 90 : 20, limit_window_seconds: 18_000 },
			secondary_window: { used_percent: 60, limit_window_seconds: 604_800 },
		} });
	});
	const older = h.run("Work");
	await new Promise((resolve) => setTimeout(resolve, 20));
	await h.run("Work");
	release();
	await older;
	await h.event("session_tree");
	assert.equal(calls, 2);
	assert.deepEqual(h.publications.at(-1), feed("5h 80% left · weekly 40% left"));
	assert.ok(!JSON.stringify(h.publications).includes("10% left"), "the stale answer never reaches the feed");
});

test("a refreshed feed formats its cached quota in the new locale without a lookup", async t => {
	const h = await setup(t);
	const locale = join(process.env.PI_CODING_AGENT_DIR!, "locale");
	const previous = process.env.OSDY_DESKTOP_LOCALE_FILE;
	process.env.OSDY_DESKTOP_LOCALE_FILE = locale;
	t.after(() => { if (previous === undefined) delete process.env.OSDY_DESKTOP_LOCALE_FILE; else process.env.OSDY_DESKTOP_LOCALE_FILE = previous; });
	await writeFile(locale, "en");
	await h.run("Work");
	await writeFile(locale, "es");
	await h.event("session_tree");
	assert.deepEqual(h.publications, [feed(null), feed("5h 75% left · weekly 40% left"), feed("5h 75% disp. · semanal 40% disp.")]);
	assert.deepEqual(h.requests, ["fixture-work"]);
	assert.deepEqual(h.notices, []);
});
