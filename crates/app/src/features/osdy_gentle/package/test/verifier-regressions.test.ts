import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { readFileSync } from "node:fs";
import { chmod, lstat, mkdir, mkdtemp, readFile, readdir, rm, symlink, unlink, utimes, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, type TestContext } from "node:test";
import { createProfile, listProfiles, readActiveAccount, readStoredCodexCredential, removeProfile, renameProfile, STORE_DIR, switchAccount, withAccountSwitchFence } from "../src/account-store.ts";
import type { PiAuthBackend } from "../src/pi-auth-backend.ts";
import { readBindings, setBinding } from "../src/bindings.ts";
import { requestProfileCodexUsage } from "../src/codex-usage.ts";
import { osdyProfileCommand, type CommandContext } from "../src/commands.ts";
import { PROFILES_SERVICE_SCHEMA } from "../src/gentle-profiles.ts";

const authBackend: PiAuthBackend = { async withLockAsync(run) { return (await run(undefined)).result; } };

async function scratch(t: TestContext) {
	const dir = await mkdtemp(join(tmpdir(), "osdy-regression-"));
	t.after(() => rm(dir, { recursive: true, force: true }));
	return dir;
}
function context(): CommandContext {
	return { isIdle: () => true, ui: { select: async () => undefined, input: async () => undefined, confirm: async () => true, notify: () => {} } };
}
test("S5: remove refuses a symlinked managed profiles directory", async (t) => {
	const agent = await scratch(t); const external = await scratch(t);
	const target = join(external, "External"); await mkdir(target);
	await mkdir(join(agent, STORE_DIR)); await symlink(external, join(agent, STORE_DIR, "profiles"), "dir");
	await assert.rejects(removeProfile(agent, "External"));
	assert.ok((await lstat(target)).isDirectory());
});
test("S5: a crashed switch-lock owner fails closed without reclaiming the fence", async (t) => {
	const agent = await scratch(t); await createProfile(agent, "First");
	const path = join(agent, STORE_DIR, "account-switch.lock");
	const child = spawn(process.execPath, ["-e", `require("node:fs").writeFileSync(process.argv[1], JSON.stringify({ pid: process.pid }), { flag: "wx", mode: 0o600 }); console.log("locked"); setInterval(() => {}, 10000);`, path]);
	t.after(() => { if (child.exitCode === null && child.signalCode === null) child.kill("SIGKILL"); });
	const exited = once(child, "exit"); await once(child.stdout!, "data"); child.kill("SIGKILL");
	assert.deepEqual(await exited, [null, "SIGKILL"]);
	const content = await readFile(path, "utf8");
	let entered = false;
	await assert.rejects(withAccountSwitchFence(agent, async () => { entered = true; }), { message: "Timed out waiting for account switch lock." });
	assert.equal(entered, false);
	assert.equal(await readFile(path, "utf8"), content);
	assert.deepEqual(await listProfiles(agent), ["First"]);
});
test("S4: switch rechecks its guard while holding the shared fence and Pi auth lock", async (t) => {
	const agent = await scratch(t);
	const canonical = join(agent, "auth.json");
	await createProfile(agent, "Home");
	await writeFile(canonical, "{}");
	const directory = join(agent, STORE_DIR);
	await chmod(directory, 0o750);
	let inPiLock = false;
	const owner: PiAuthBackend = { async withLockAsync(run) { inPiLock = true; try { return (await run(undefined)).result; } finally { inPiLock = false; } } };
	let checks = 0;
	await assert.rejects(switchAccount(agent, "Home", { authBackend: owner, guard: () => {
		checks++;
		assert.equal(inPiLock, true);
		assert.equal(readFileSync(join(directory, "account-switch.lock"), "utf8"), `${JSON.stringify({ pid: process.pid })}\n`);
		return false;
	} }), { message: "The agent is busy; nothing was switched. Try again when it is idle." });
	assert.equal(checks, 1);
	assert.equal(await readFile(canonical, "utf8"), "{}");
	assert.equal((await lstat(directory)).mode & 0o777, 0o750);
	assert.deepEqual(await readdir(join(directory, "profiles", "Home")), []);
	assert.deepEqual(await readActiveAccount(agent), { status: "unset" });
});

test("S4: becoming busy while waiting for the switch lock prevents switching", async (t) => {
	const agentDir = await scratch(t); await createProfile(agentDir, "Alpha"); await createProfile(agentDir, "Beta");
	await switchAccount(agentDir, "Alpha", { authBackend });
	const auth = '{"synthetic":"alpha"}\n';
	await writeFile(join(agentDir, "auth.json"), auth);
	const store = join(agentDir, STORE_DIR);
	await chmod(store, 0o750);
	const lock = join(store, "account-switch.lock");
	await writeFile(lock, JSON.stringify({ pid: process.pid }), { flag: "wx" });
	let idle = true; let calls = 0; let checked!: () => void;
	let observedLock: string | undefined;
	const checkedIdle = new Promise<void>((resolve) => { checked = resolve; });
	const ctx = context(); ctx.isIdle = () => {
		if (++calls === 2) checked();
		if (calls === 3) observedLock = readFileSync(lock, "utf8");
		return idle;
	};
	const notes: unknown[] = [];
	ctx.ui.notify = (message, type) => notes.push({ message, type });
	const deps = { agentDir, events: { emit() {} }, authBackend };
	const pending = osdyProfileCommand("Beta", ctx, deps);
	await checkedIdle; idle = false; await unlink(lock); await pending;
	assert.deepEqual(await readActiveAccount(agentDir), { status: "valid", profile: "Alpha" });
	assert.equal(await readFile(join(agentDir, "auth.json"), "utf8"), auth);
	assert.deepEqual(await readdir(join(agentDir, STORE_DIR, "profiles", "Alpha")), []);
	assert.deepEqual(notes, [{ message: "The agent is busy; nothing was switched. Try again when it is idle.", type: "warning" }]);
	assert.equal(observedLock, `${JSON.stringify({ pid: process.pid })}\n`);
	assert.equal((await lstat(store)).mode & 0o777, 0o750);
	await assert.rejects(lstat(lock), { code: "ENOENT" });
});
test("S3: owner rejection reports partial success and disposes", async (t) => {
	const agentDir = await scratch(t); await createProfile(agentDir, "Beta"); await setBinding(agentDir, "Beta", "fast");
	const notes: string[] = []; const levels: unknown[] = []; const ctx = context();
	ctx.ui.notify = (message, level) => { notes.push(message); levels.push(level); };
	let disposed = 0;
	const deps = { agentDir, events: { emit() {} }, authBackend, discover: async () => ({ schema: PROFILES_SERVICE_SCHEMA, apply: async () => { throw new Error("owner rejected"); }, dispose: () => { disposed++; } }) };
	await osdyProfileCommand("Beta", ctx, deps);
	assert.deepEqual(await readActiveAccount(agentDir), { status: "valid", profile: "Beta" });
	assert.equal(disposed, 1);
	assert.match(notes.at(-1) ?? "", /Account switched to Beta\..*not applied/);
	assert.deepEqual(notes, ['Account switched to Beta. Model profile "fast" was not applied: Gentle rejected the request.']);
	assert.deepEqual(levels, ["warning"]);
});
test("S8: a credential expiring during its read is never sent", async () => {
	const now = 1_800_000_000_000;
	const b64 = (value: unknown) => Buffer.from(JSON.stringify(value)).toString("base64url");
	const claims = { exp: now / 1000 + 3600, "https://api.openai.com/auth": { chatgpt_account_id: "test-account" } };
	for (const [expires, exp] of [[now + 1, claims.exp], [now + 60_000, now / 1000 + 1]]) {
		let clock = now; let sent = 0;
		const result = await requestProfileCodexUsage("Alpha", async (profile) => {
			clock += expires === now + 1 ? 2 : 1000;
			return { profile, expires, access: `${b64({ alg: "none" })}.${b64({ ...claims, exp })}.synthetic` };
		}, { now: () => clock, fetch: async () => { sent++; return new Response("{}"); } });
		assert.equal(sent, 0);
		assert.deepEqual(result, { status: "unavailable", profile: "Alpha", reason: "stored-credentials-unavailable" });
	}
});
test("S5: listing and switching refuse a symlinked profiles directory", async (t) => {
	const agent = await scratch(t); const external = await scratch(t);
	const target = join(external, "External"); await mkdir(target);
	await writeFile(join(target, "auth.json"), '{"synthetic":"external"}');
	await writeFile(join(agent, "auth.json"), '{"synthetic":"local"}');
	await mkdir(join(agent, STORE_DIR)); await symlink(external, join(agent, STORE_DIR, "profiles"), "dir");
	const listed = listProfiles(agent); const switched = switchAccount(agent, "External", { authBackend });
	await Promise.all([assert.rejects(listed, /Managed path is not a directory/), assert.rejects(switched, /Managed path is not a directory/)]);
	assert.deepEqual(await readdir(target), ["auth.json"]);
	assert.equal(await readFile(join(target, "auth.json"), "utf8"), '{"synthetic":"external"}');
	assert.equal(await readFile(join(agent, "auth.json"), "utf8"), '{"synthetic":"local"}');
	assert.deepEqual(await readActiveAccount(agent), { status: "unset" });
});
test("S5: binding reads and writes reject a symlinked store without touching it", async (t) => {
	const agent = await scratch(t); const external = await scratch(t);
	await symlink(external, join(agent, STORE_DIR), "dir");
	await Promise.all([
		assert.rejects(setBinding(agent, "Alpha", "fast"), /Managed path is not a directory/),
		assert.rejects(readBindings(agent), /Managed path is not a directory/),
	]);
	assert.deepEqual(await readdir(external), []);
});
test("S5: single-profile symlinks are rejected rather than followed or listed", async (t) => {
	const agent = await scratch(t); const external = await scratch(t);
	await createProfile(agent, "First"); await switchAccount(agent, "First", { authBackend });
	const target = join(agent, STORE_DIR, "profiles", "Trap");
	await symlink(external, target, "dir");
	await assert.rejects(listProfiles(agent), /Managed path is not a directory/);
	await assert.rejects(readActiveAccount(agent), /Managed path is not a directory/);
	await assert.rejects(readStoredCodexCredential(agent, "Trap"), /Managed path is not a directory/);
	await assert.rejects(createProfile(agent, "Trap"), /Managed path is not a directory/);
	await assert.rejects(renameProfile(agent, "First", "Trap"), /Managed path is not a directory/);
	assert.ok((await lstat(target)).isSymbolicLink());
	assert.deepEqual(await readdir(external), []);
});
test("S5: a symlinked backups directory cannot receive an unowned sign-in", async (t) => {
	const agent = await scratch(t); const external = await scratch(t);
	await createProfile(agent, "First");
	const auth = '{"synthetic":"unowned"}';
	await writeFile(join(agent, "auth.json"), auth);
	await symlink(external, join(agent, STORE_DIR, "backups"), "dir");
	await assert.rejects(switchAccount(agent, "First", { authBackend }), /Managed path is not a directory/);
	assert.equal(await readFile(join(agent, "auth.json"), "utf8"), auth);
	assert.deepEqual(await readActiveAccount(agent), { status: "unset" });
	assert.deepEqual(await readdir(external), []);
});
test("S5: a live lock owner still times out without changing the store", async (t) => {
	const agent = await scratch(t); await createProfile(agent, "First");
	const lock = join(agent, STORE_DIR, "account-switch.lock");
	const contents = JSON.stringify({ pid: process.pid });
	await writeFile(lock, contents, { flag: "wx" });
	const old = new Date(Date.now() - 20_000);
	await utimes(lock, old, old);
	await assert.rejects(createProfile(agent, "Blocked"), { message: "Timed out waiting for account switch lock." });
	assert.equal(await readFile(lock, "utf8"), contents);
	assert.deepEqual(await listProfiles(agent), ["First"]);
});
test("S5: old malformed locks are not reclaimed by competing writers", async (t) => {
	const agent = await scratch(t); await createProfile(agent, "First");
	const lock = join(agent, STORE_DIR, "account-switch.lock");
	await writeFile(lock, "broken", { flag: "wx" });
	const old = new Date(Date.now() - 20_000);
	await utimes(lock, old, old);
	await assert.rejects(createProfile(agent, "Second"), { message: "Timed out waiting for account switch lock." });
	assert.equal(await readFile(lock, "utf8"), "broken");
	assert.deepEqual(await listProfiles(agent), ["First"]);
});
