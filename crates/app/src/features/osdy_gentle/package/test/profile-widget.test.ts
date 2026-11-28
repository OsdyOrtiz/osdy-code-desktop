import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, type TestContext } from "node:test";
import type { ActiveAccount } from "../src/account-store.ts";
import osdyGentle from "../src/index.ts";
import { registerProfileWidget, type WidgetHost } from "../src/profile-widget.ts";

const KEY = "osdy-gentle:codex-accounts";

// Each publish as [key, the feed's active account] (undefined: cleared).
function actives(calls: [string, string[] | undefined][]) {
	return calls.map(([key, lines]) => [key, lines === undefined ? undefined : JSON.parse(lines[0]).active]);
}

async function storeWith(t: TestContext, names: string[]) {
	const dir = await mkdtemp(join(tmpdir(), "osdy-profile-widget-"));
	t.after(() => rm(dir, { recursive: true, force: true }));
	for (const name of names) await mkdir(join(dir, "osdy-gentle", "profiles", name), { recursive: true });
	return dir;
}
const longName = "Engineering-Development";

function fakeHost() {
	const handlers = new Map<Parameters<NonNullable<WidgetHost["on"]>>[0], Parameters<NonNullable<WidgetHost["on"]>>[1]>();
	const host = {
		on: ((event, handler) => {
			handlers.set(event, handler);
			return () => { handlers.delete(event); };
		}) satisfies NonNullable<WidgetHost["on"]>,
	};
	return { host, handlers };
}

function callsFromUI() {
	const calls: [string, string[] | undefined][] = [];
	const ui = { setWidget: (key: string, lines: string[] | undefined) => { calls.push([key, lines]); } };
	return { calls, ui };
}

function fakePoll(t: TestContext) {
	const stopped: unknown[] = [];
	const timers: { tick: () => unknown; delay: number; unrefs: number; unref(): void }[] = [];
	t.mock.method(globalThis, "setInterval", (tick: () => unknown, delay: number) => {
		const timer = { tick, delay, unrefs: 0, unref() { this.unrefs++; } };
		timers.push(timer);
		return timer;
	});
	t.mock.method(globalThis, "clearInterval", (timer: unknown) => { stopped.push(timer); });
	return { timers, stopped };
}

test("the extension wires the Pi RPC widget lifecycle alongside its commands", () => {
	const { host, handlers } = fakeHost();
	const commands: string[] = [];
	osdyGentle({ ...host, events: { emit: () => {} }, registerCommand: (name) => { commands.push(name); } });
	assert.deepEqual(commands, ["osdy-profile", "osdy-account", "osdy-auth-status", "osdy-usage"]);
	assert.deepEqual([...handlers.keys()].sort(), ["session_compact", "session_shutdown", "session_start", "session_tree"]);
});

test("publishes only in RPC and refreshes the active metadata on polls and session boundaries", async (t) => {
	const { host, handlers } = fakeHost();
	const { calls, ui } = callsFromUI();
	const { timers, stopped } = fakePoll(t);
	const dir = await mkdtemp(join(tmpdir(), "osdy-profile-widget-"));
	t.after(() => rm(dir, { recursive: true, force: true }));
	const store = join(dir, "osdy-gentle");
	const metadata = join(store, "active-account.json");
	for (const name of ["Work", "Home", longName]) {
		await mkdir(join(store, "profiles", name), { recursive: true });
	}
	const setActive = (profile: string) => writeFile(metadata, JSON.stringify({ version: 1, profile }));
	await setActive("work");
	registerProfileWidget(host, () => dir);
	const start = handlers.get("session_start");
	assert.ok(start, "must listen for Pi's session_start");
	await start({}, { mode: "tui", ui });
	assert.deepEqual(calls, []);
	assert.equal(timers.length, 0);

	await start({ reason: "startup" }, { mode: "rpc", ui });
	assert.deepEqual(actives(calls), [[KEY, "Work"]]);
	assert.equal(timers.length, 1);
	assert.equal(timers[0].unrefs, 1);
	assert.ok(timers[0].delay >= 1_000 && timers[0].delay <= 5_000);
	await timers[0].tick();
	assert.equal(calls.length, 1, "unchanged state should not emit RPC traffic");

	await setActive("Home");
	await timers[0].tick();
	assert.deepEqual(actives(calls.splice(1)), [[KEY, "Home"]]);

	await setActive(longName);
	const tree = handlers.get("session_tree");
	assert.ok(tree);
	await tree({}, { mode: "rpc", ui });
	assert.deepEqual(actives(calls.splice(1)), [[KEY, longName]]);

	await setActive("Work\nInjected");
	const compact = handlers.get("session_compact");
	assert.ok(compact);
	await compact({}, { mode: "rpc", ui });
	assert.deepEqual(actives(calls.splice(1)), [[KEY, null]]);

	await setActive("Home");
	await timers[0].tick();
	assert.deepEqual(actives(calls.splice(1)), [[KEY, "Home"]]);
	const stop = handlers.get("session_shutdown");
	assert.ok(stop);
	await stop({ reason: "resume" }, { mode: "rpc", ui });
	assert.deepEqual(stopped, [timers[0]]);
	assert.deepEqual(actives(calls.splice(1)), [[KEY, undefined]]);
	const next = callsFromUI();
	await start({ reason: "resume" }, { mode: "rpc", ui: next.ui });
	assert.deepEqual(actives(next.calls), [[KEY, "Home"]]);
	assert.equal(timers.length, 2);
	await stop({ reason: "quit" }, { mode: "rpc", ui: next.ui });
	assert.deepEqual(stopped, timers);
});

test("clears a published name on missing metadata or a failed read and can recover", async (t) => {
	const { host, handlers } = fakeHost();
	const { calls, ui } = callsFromUI();
	const { timers } = fakePoll(t);
	const ctx = { mode: "rpc", ui };
	let state: ActiveAccount = { status: "valid", profile: "Work" };
	let fail = false;
	const dir = await storeWith(t, ["Work", "Home"]);
	registerProfileWidget(host, () => dir, async () => {
		if (fail) throw new Error("metadata read failed");
		return state;
	});
	const start = handlers.get("session_start");
	assert.ok(start);
	await start({}, ctx);
	state = { status: "unset" };
	await timers[0].tick();
	state = { status: "valid", profile: "Home" };
	await timers[0].tick();
	fail = true;
	await timers[0].tick();
	assert.deepEqual(actives(calls), [[KEY, "Work"], [KEY, null], [KEY, "Home"], [KEY, null]]);
	await handlers.get("session_shutdown")?.({}, ctx);
});

test("never overlaps reads or publishes superseded results, including across shutdown", async (t) => {
	const { host, handlers } = fakeHost();
	const first = callsFromUI();
	const next = callsFromUI();
	const { timers } = fakePoll(t);
	const reads: ((result: ActiveAccount) => void)[] = [];
	const dir = await storeWith(t, ["Work", "Home", "Stale", "Discarded"]);
	registerProfileWidget(host, () => dir, () => new Promise((resolve) => { reads.push(resolve); }));
	const start = handlers.get("session_start");
	const tree = handlers.get("session_tree");
	const stop = handlers.get("session_shutdown");
	assert.ok(start && tree && stop);
	const dispatch = (handler: NonNullable<typeof start>, ui: typeof first.ui) => handler({}, { mode: "rpc", ui });
	// Lets in-flight store reads (real file I/O) settle, not just microtasks.
	const drain = () => new Promise<void>((resolve) => { setTimeout(resolve, 20); });
	const started = dispatch(start, first.ui);
	await drain();
	reads[0]({ status: "valid", profile: "Work" });
	await started;

	const poll = timers[0].tick();
	await drain();
	const boundary = dispatch(tree, first.ui);
	void timers[0].tick();
	await drain();
	assert.equal(reads.length, 2);
	reads[1]({ status: "valid", profile: "Stale" });
	await drain();
	assert.equal(reads.length, 3);
	assert.deepEqual(actives(first.calls), [[KEY, "Work"]]);
	await dispatch(stop, first.ui);
	const restarted = dispatch(start, next.ui);
	await drain();
	assert.equal(reads.length, 3);
	reads[2]({ status: "valid", profile: "Discarded" });
	await drain();
	assert.equal(reads.length, 4);
	assert.deepEqual(next.calls, []);
	reads[3]({ status: "valid", profile: "Home" });
	await Promise.all([restarted, boundary, poll]);
	assert.deepEqual(actives(first.calls), [[KEY, "Work"], [KEY, undefined]]);
	assert.deepEqual(actives(next.calls), [[KEY, "Home"]]);
	await dispatch(stop, next.ui);
});
