import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import fsPromises, { mkdtemp, readdir, readFile, writeFile, rm } from "node:fs/promises";
import { registerHooks, syncBuiltinESMExports } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createProfile, readActiveAccount, saveCurrentAs } from "../src/account-store.ts";
import { setBinding } from "../src/bindings.ts";
import { osdyAccountCommand, osdyProfileCommand, type CommandContext, type Deps } from "../src/commands.ts";
import { PROFILES_SERVICE_SCHEMA, type ProfilesService } from "../src/gentle-profiles.ts";
import osdyGentle from "../src/index.ts";
import type { PiAuthBackend } from "../src/pi-auth-backend.ts";
import type { WidgetHost } from "../src/profile-widget.ts";

const authBackend: PiAuthBackend = { async withLockAsync(run) { return (await run(undefined)).result; } };

test("reset menu only runs the explicitly selected active-account action", async () => {
	const calls: string[] = [];
	const resetActions = {
		redeem: async () => { calls.push("redeem"); return "cancelled" as const; },
		check: async () => { calls.push("check"); return "pending" as const; },
	};
	const deps = { agentDir: "unused", events: { emit: () => {} }, resetActions };
	for (const [selection, expected] of [
		[undefined, []],
		["Check last reset attempt", ["check"]],
		["Use a banked reset", ["check", "redeem"]],
	] as const) {
		const { ctx, selects } = fakeCtx({ selects: ["Use or check banked reset", selection] });
		await osdyAccountCommand("", ctx, deps);
		assert.ok(selects[0]?.includes("Use or check banked reset"));
		assert.deepEqual(selects[1], ["Cancel", "Use a banked reset", "Check last reset attempt"]);
		assert.deepEqual(calls, expected);
	}
});

const auth = (tag: string) => `${JSON.stringify({ "openai-codex": { type: "oauth", access: `x.${tag}.y`, refresh: "r", expires: 1 } })}\n`;

type Script = { selects?: (string | ((options: string[]) => string | undefined))[]; confirms?: boolean[]; inputs?: (string | undefined)[]; idle?: boolean[] };

function fakeCtx(script: Script) {
	const notes: { message: string; type?: string }[] = [];
	const confirms: string[] = [];
	const selects: string[][] = [];
	const idle = [...(script.idle ?? [true])];
	const ctx: CommandContext = {
		ui: {
			select: async (_t, options) => {
				selects.push(options);
				const next = script.selects?.shift();
				return typeof next === "function" ? next(options) : next;
			},
			confirm: async (t, m) => {
				confirms.push(`${t}: ${m}`);
				return script.confirms?.shift() ?? false;
			},
			input: async () => script.inputs?.shift(),
			notify: (message, type) => notes.push({ message, type }),
		},
		isIdle: () => (idle.length > 1 ? idle.shift()! : idle[0]!),
		waitForIdle: async () => {},
	};
	return { ctx, notes, confirms, selects };
}

async function setup() {
	const dir = await mkdtemp(join(tmpdir(), "osdy-gentle-c-"));
	await writeFile(join(dir, "auth.json"), auth("work"));
	await saveCurrentAs(dir, "Work", { authBackend });
	await createProfile(dir, "Home");
	await writeFile(join(dir, "osdy-gentle/profiles/Home/auth.json"), auth("home"));
	return dir;
}

function service(calls: unknown[]): ProfilesService {
	return {
		schema: PROFILES_SERVICE_SCHEMA,
		apply: async (r) => {
			calls.push(r);
			return { status: "applied", notes: [] };
		},
		dispose: () => calls.push("disposed"),
	};
}

const noFetch = (async () => new Response("", { status: 500 })) as unknown as typeof fetch;

test("/osdy-profile <name> switches account and applies the linked model profile", async () => {
	const dir = await setup();
	await setBinding(dir, "Home", "fast");
	const calls: unknown[] = [];
	const deps: Deps = { agentDir: dir, events: { emit: () => {} }, authBackend, discover: async () => service(calls) };
	const { ctx, notes, confirms } = fakeCtx({ confirms: [true] });
	await osdyProfileCommand("home", ctx, deps);
	assert.match(confirms[0]!, /account Home and model profile "fast".*every Gentle Shell tab.*Other providers stay signed in/);
	assert.deepEqual(JSON.parse(await readFile(join(dir, "auth.json"), "utf8")), JSON.parse(auth("home")));
	assert.deepEqual(calls, [{ profile: "fast", mode: "session" }, "disposed"]);
	assert.deepEqual(notes.at(-1), { message: 'Account switched to Home. Model profile "fast" applied to this session.', type: "info" });
	await rm(dir, { recursive: true });
});

test("the account switch menu reports a full-file backup when only Claude was signed in", async t => {
	const dir = await mkdtemp(join(tmpdir(), "osdy-gentle-c-"));
	t.after(() => rm(dir, { recursive: true, force: true }));
	const anthropic = { type: "oauth", access: "fake-claude", refresh: "fake-refresh", expires: 1 };
	const claudeOnly = `${JSON.stringify({ anthropic }, null, 2)}\n`;
	await writeFile(join(dir, "auth.json"), claudeOnly);
	await createProfile(dir, "Home");
	await writeFile(join(dir, "osdy-gentle/profiles/Home/auth.json"), auth("home"));
	const { ctx, notes, confirms } = fakeCtx({ selects: ["Switch profile", options => options[0]], confirms: [true] });
	await osdyAccountCommand("", ctx, { agentDir: dir, events: { emit: () => {} }, authBackend, fetch: noFetch });
	const backups = join(dir, "osdy-gentle/backups");
	const paths = await readdir(backups);
	assert.equal(paths.length, 1);
	const backup = join(backups, paths[0]!);
	assert.equal(await readFile(backup, "utf8"), claudeOnly);
	assert.deepEqual(JSON.parse(await readFile(join(dir, "auth.json"), "utf8")), { anthropic, ...JSON.parse(auth("home")) });
	assert.deepEqual(await readActiveAccount(dir), { status: "valid", profile: "Home" });
	assert.deepEqual(confirms, ["Switch profile: Switch to account Home? The account changes for every Gentle Shell tab. Other providers stay signed in."]);
	assert.deepEqual(notes, [
		{ message: "Checking Codex quota for stored accounts…", type: "info" },
		{ message: `Account switched to Home. Previous unowned sign-in saved to ${backup}.`, type: "info" },
	]);
});

test("declining the confirmation or a busy agent leaves the account unchanged", async () => {
	const dir = await setup();
	const deps: Deps = { agentDir: dir, events: { emit: () => {} }, authBackend };
	const declined = fakeCtx({ confirms: [false] });
	await osdyProfileCommand("Home", declined.ctx, deps);
	const busy = fakeCtx({ confirms: [true], idle: [false] });
	await osdyProfileCommand("Home", busy.ctx, deps);
	assert.equal(busy.notes.at(-1)?.message, "The agent is busy; nothing was switched. Try again when it is idle.");
	assert.equal(await readFile(join(dir, "auth.json"), "utf8"), auth("work"));
	assert.deepEqual(await readActiveAccount(dir), { status: "valid", profile: "Work" });
	await rm(dir, { recursive: true });
});

test("without a Gentle profiles owner the user may switch the account only", async () => {
	const dir = await setup();
	await setBinding(dir, "Home", "fast");
	const deps: Deps = { agentDir: dir, events: { emit: () => {} }, authBackend, discover: async () => undefined };
	const { ctx, notes, confirms } = fakeCtx({ confirms: [true, true] });
	await osdyProfileCommand("Home", ctx, deps);
	assert.match(confirms[0]!, /Gentle profiles unavailable/);
	assert.equal(notes.at(-1)?.message, "Account switched to Home.");
	assert.deepEqual(JSON.parse(await readFile(join(dir, "auth.json"), "utf8")), JSON.parse(auth("home")));
	await rm(dir, { recursive: true });
});

test("unknown account argument is an error and changes nothing", async () => {
	const dir = await setup();
	const { ctx, notes } = fakeCtx({});
	await osdyProfileCommand("Nope", ctx, { agentDir: dir, events: { emit: () => {} } });
	assert.deepEqual(notes, [{ message: "Unknown account: Nope", type: "error" }]);
	assert.equal(await readFile(join(dir, "auth.json"), "utf8"), auth("work"));
	await rm(dir, { recursive: true });
});

test("picker labels show active marker, linked model and quota", async () => {
	const dir = await setup();
	await setBinding(dir, "Work", "balanced");
	const { ctx, selects } = fakeCtx({ selects: [undefined] });
	await osdyProfileCommand("", ctx, { agentDir: dir, events: { emit: () => {} }, fetch: noFetch });
	assert.deepEqual(selects[0], ["Home — quota: no valid stored token", "Work (active) — model: balanced — quota: no valid stored token"]);
	await rm(dir, { recursive: true });
});

test("/osdy-account links a model profile chosen from Gentle's profiles.json", async () => {
	const dir = await setup();
	const config = await mkdtemp(join(tmpdir(), "osdy-gentle-cfg-"));
	await writeFile(join(config, "profiles.json"), JSON.stringify({ kind: "gentle-pi.agent_model_profiles", version: 1, profiles: { fast: {} } }));
	const { ctx, notes } = fakeCtx({ selects: ["Link model profile to an account", (o) => o.find((x) => x.startsWith("Home")), "fast"] });
	await osdyAccountCommand("", ctx, { agentDir: dir, events: { emit: () => {} }, gentleConfigHome: config });
	assert.equal(notes.at(-1)?.message, 'Home now applies model profile "fast".');
	const saved = JSON.parse(await readFile(join(dir, "osdy-gentle/bindings.json"), "utf8"));
	assert.deepEqual(saved, { version: 1, bindings: { home: { gentleProfile: "fast" } } });
	await rm(dir, { recursive: true });
	await rm(config, { recursive: true });
});

test("/osdy-auth-status is listed and emits only sanitized metadata without command I/O", async t => {
	const sdk = "@earendil-works/pi-coding-agent";
	let sdkModule: string | undefined;
	const hook = registerHooks({ resolve(specifier, context, next) {
		if (specifier !== sdk) return next(specifier, context);
		if (!sdkModule) throw new Error("private SDK import error");
		return next(`data:text/javascript,${encodeURIComponent(sdkModule)}`, context);
	} });
	t.after(() => hook.deregister());
	const effects: string[] = [];
	const forbidden = (action: string) => () => { effects.push(action); assert.fail(`unexpected ${action}`); };
	for (const key of ["existsSync", "lstatSync", "unlinkSync"] as const) t.mock.method(fs, key, forbidden(key));
	for (const key of ["readFile", "readdir", "open", "lstat", "chmod", "mkdir", "rename", "rmdir", "unlink", "writeFile"] as const) {
		t.mock.method(fsPromises, key, forbidden(key));
	}
	t.mock.method(globalThis, "fetch", forbidden("fetch"));
	syncBuiltinESMExports();
	try {
		for (const [source, stage, version] of [
			[undefined, "sdk-import", "unknown"],
			['export const VERSION = "1.6.0 private\\nvalue";', "sdk-version", "unknown"],
			['export const VERSION = "1.6.0"; export const getAgentDir = () => "/private/synthetic-agent";', "inspection-exception", "1.6.0"],
		] as const) {
			sdkModule = source;
			const commands = new Map<string, Parameters<Parameters<typeof osdyGentle>[0]["registerCommand"]>[1]>();
			osdyGentle({ events: { emit: forbidden("events.emit") }, registerCommand: (name, options) => commands.set(name, options) });
			assert.deepEqual([...commands.keys()], ["osdy-profile", "osdy-account", "osdy-auth-status", "osdy-usage"]);
			const { ctx, notes, selects, confirms } = fakeCtx({});
			ctx.isIdle = forbidden("isIdle");
			ctx.waitForIdle = forbidden("waitForIdle");
			ctx.ui.input = forbidden("input");
			let inspected = 0;
			Object.defineProperty(ctx, "modelRegistry", { get() { inspected++; throw new Error("private registry error"); } });
			await commands.get("osdy-auth-status")!.handler("ignored private arg", ctx);
			assert.deepEqual(notes, [{ message: `osdy-auth/v1 sdk=${version} stage=${stage} storePath=unchecked backendPath=unchecked`, type: "info" }]);
			assert.equal(inspected, stage === "inspection-exception" ? 1 : 0);
			assert.deepEqual(selects, []);
			assert.deepEqual(confirms, []);
		}
		assert.deepEqual(effects, []);
	} finally {
		t.mock.restoreAll();
		syncBuiltinESMExports();
	}
});

test("/osdy-profile without a verified owner keeps its fail-closed message and account", async t => {
	const dir = await setup();
	t.after(() => rm(dir, { recursive: true, force: true }));
	const { ctx, confirms, notes } = fakeCtx({ confirms: [true] });
	await osdyProfileCommand("Home", ctx, { agentDir: dir, events: { emit: () => assert.fail("not switched") }, fetch: noFetch });
	assert.equal(confirms.length, 1);
	assert.deepEqual(notes, [{ message: "Pi's auth lock is unavailable; no account changes were made.", type: "error" }]);
	assert.equal(await readFile(join(dir, "auth.json"), "utf8"), auth("work"));
	assert.equal(await readFile(join(dir, "osdy-gentle/profiles/Home/auth.json"), "utf8"), auth("home"));
	assert.deepEqual(await readActiveAccount(dir), { status: "valid", profile: "Work" });
});

test("extension registers commands and retires each session controller on start or shutdown", async t => {
	const names: string[] = [];
	type Subscribe = NonNullable<WidgetHost["on"]>;
	type Handler = Parameters<Subscribe>[1];
	const handlers = new Map<Parameters<Subscribe>[0], Handler[]>();
	const on: Subscribe = (event, handler) => {
		handlers.set(event, [...(handlers.get(event) ?? []), handler]);
		return () => {};
	};
	const abort = AbortController.prototype.abort;
	const retired: AbortSignal[] = [];
	t.mock.method(AbortController.prototype, "abort", function (this: AbortController) {
		retired.push(this.signal);
		abort.call(this);
	});
	osdyGentle({ on, events: { emit: () => {} }, registerCommand: (name) => names.push(name) });
	assert.deepEqual(names, ["osdy-profile", "osdy-account", "osdy-auth-status", "osdy-usage"]);
	const ctx = { mode: "tui", ui: { setWidget: () => assert.fail("unexpected TUI widget") } };
	for (const event of ["session_start", "session_start", "session_shutdown"] as const) {
		assert.equal(handlers.get(event)?.length, 2, "widget and reset listeners must coexist");
		for (const handler of handlers.get(event)!) await handler({}, ctx);
	}
	assert.equal(retired.length, 3);
	assert.equal(new Set(retired).size, 3);
	assert.ok(retired.every(signal => signal.aborted));
});

test("Spanish account menu uses its displayed choice and leaves a declined import untouched", async t => {
	const dir = await mkdtemp(join(tmpdir(), "osdy-locale-command-"));
	t.after(() => rm(dir, { recursive: true, force: true }));
	const locale = join(dir, "locale");
	await writeFile(locale, "es");
	const previous = process.env.OSDY_DESKTOP_LOCALE_FILE;
	process.env.OSDY_DESKTOP_LOCALE_FILE = locale;
	t.after(() => { if (previous === undefined) delete process.env.OSDY_DESKTOP_LOCALE_FILE; else process.env.OSDY_DESKTOP_LOCALE_FILE = previous; });
	const deps = { agentDir: dir, osdySharedDir: dir, events: { emit() {} } };
	const { ctx, notes, confirms, selects } = fakeCtx({ selects: [options => options.at(-1)] });
	await osdyAccountCommand("", ctx, deps);
	assert.equal(selects[0]?.at(-1), "Importar cuentas de Osdy-Pi");
	assert.deepEqual(confirms, [`Importar cuentas de Osdy-Pi: ¿Copiar las cuentas guardadas de ${dir}/osdy-pi/profiles a Gentle Shell? Se conservan los nombres existentes y no se modifica Osdy-Pi.`]);
	await osdyProfileCommand("Work", ctx, deps);
	assert.deepEqual(notes, [{ message: "Cuenta desconocida: Work", type: "error" }]);
	assert.deepEqual(await readdir(dir), ["locale"]);
});
