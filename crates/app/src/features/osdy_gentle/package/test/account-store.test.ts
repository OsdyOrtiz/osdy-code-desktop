import { test, type TestContext } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, readdir, stat, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
	createProfile, importOsdyProfiles, listProfiles, readActiveAccount,
	readStoredCodexCredential, removeProfile, renameProfile,
	saveCurrentAs as saveWithOptions, switchAccount as switchWithOptions, validateProfileName,
} from "../src/account-store.ts";
import type { PiAuthBackend } from "../src/pi-auth-backend.ts";

const codex = (tag: string) => ({ type: "oauth", access: `a.${tag}.c`, refresh: `r-${tag}`, expires: 9e15 });
const auth = (tag: string) => `${JSON.stringify({ "openai-codex": codex(tag), anthropic: { type: "api_key", key: tag } })}\n`;
const authBackend: PiAuthBackend = { async withLockAsync(run) { return (await run(undefined)).result; } };
const saveCurrentAs = (dir: string, name: string) => saveWithOptions(dir, name, { authBackend });
const switchAccount = (dir: string, name: string) => switchWithOptions(dir, name, { authBackend });

async function tempAgent(t: TestContext): Promise<string> {
	const dir = await mkdtemp(join(tmpdir(), "osdy-gentle-"));
	t.after(() => rm(dir, { recursive: true }));
	return dir;
}

const read = (p: string) => readFile(p, "utf8");

test("rejects invalid and reserved profile names", () => {
	for (const bad of ["", "../x", "a b", "default", "Profiles", "-x", "a".repeat(64)])
		assert.throws(() => validateProfileName(bad), /Profile names must use/);
	assert.equal(validateProfileName("Osdy-Work"), "Osdy-Work");
});

test("saveCurrentAs snapshots only signed-in Codex and marks it active", async (t) => {
	const dir = await tempAgent(t);
	const canonical = join(dir, "auth.json");
	const snapshot = join(dir, "osdy-gentle/profiles/Work/auth.json");
	await writeFile(canonical, auth("work"));
	assert.equal(await saveCurrentAs(dir, "Work"), "Work");
	assert.deepEqual(await listProfiles(dir), ["Work"]);
	assert.deepEqual(JSON.parse(await read(snapshot)), { "openai-codex": codex("work") });
	assert.deepEqual(await readActiveAccount(dir), { status: "valid", profile: "Work" });
	assert.equal((await stat(snapshot)).mode & 0o777, 0o600);
	await assert.rejects(saveCurrentAs(dir, "work"), /already exists/);
	assert.equal(await read(canonical), auth("work"));
});

test("save rejects absent or incomplete Codex OAuth without creating an account", async (t) => {
	const dir = await tempAgent(t);
	const canonical = join(dir, "auth.json");
	for (const [document, message] of [
		['{"anthropic":{"type":"api_key","key":"live"}}', "There is no signed-in Codex account to save."],
		['{"openai-codex":{"type":"oauth","access":"incomplete"}}', "Current auth.json contains invalid Codex credentials."],
	]) {
		await writeFile(canonical, document);
		await assert.rejects(saveCurrentAs(dir, "Work"), { message });
		assert.deepEqual(await listProfiles(dir), []);
		assert.deepEqual(await readActiveAccount(dir), { status: "unset" });
		assert.equal(await read(canonical), document);
	}
});

test("switch and save fail closed without an injected Pi auth owner", async (t) => {
	const dir = await tempAgent(t);
	const canonical = join(dir, "auth.json");
	await writeFile(canonical, auth("live"));
	await createProfile(dir, "Home");
	const message = "Pi's auth lock is unavailable; no account changes were made.";
	await assert.rejects(saveWithOptions(dir, "Work"), { message });
	await assert.rejects(switchWithOptions(dir, "Home"), { message });
	assert.equal(await read(canonical), auth("live"));
	assert.deepEqual(await listProfiles(dir), ["Home"]);
	assert.deepEqual(await readActiveAccount(dir), { status: "unset" });
	assert.deepEqual(await readdir(join(dir, "osdy-gentle/profiles/Home")), []);
});

test("switch preserves the live Claude sign-in", async (t) => {
	const dir = await tempAgent(t);
	const canonical = join(dir, "auth.json");
	const live = { ...JSON.parse(auth("work")), other: { type: "api_key", key: "live-other" } };
	const incoming = JSON.parse(auth("home"));
	await writeFile(canonical, JSON.stringify(live));
	await saveCurrentAs(dir, "Work");
	await createProfile(dir, "Home");
	await writeFile(join(dir, "osdy-gentle/profiles/Home/auth.json"), JSON.stringify(incoming));

	assert.deepEqual(await switchAccount(dir, "Home"), { profile: "Home", unchanged: false });
	const switched = JSON.parse(await read(canonical));
	assert.deepEqual(switched.anthropic, live.anthropic);
	assert.deepEqual(switched.other, live.other);
	assert.deepEqual(switched["openai-codex"], incoming["openai-codex"]);
	assert.deepEqual(await readActiveAccount(dir), { status: "valid", profile: "Home" });
});

test("switch reads and commits inside the injected Pi lock without a second backend write", async (t) => {
	const dir = await tempAgent(t);
	const canonical = join(dir, "auth.json");
	await writeFile(canonical, auth("before-lock"));
	await createProfile(dir, "Empty");
	const locked: PiAuthBackend = {
		async withLockAsync(run) {
			await writeFile(canonical, auth("pi-refresh"));
			const result = await run(auth("pi-refresh"));
			assert.deepEqual(Object.keys(result), ["result"]);
			assert.deepEqual(JSON.parse(await read(canonical)), { anthropic: { type: "api_key", key: "pi-refresh" } });
			assert.deepEqual(await readActiveAccount(dir), { status: "valid", profile: "Empty" });
			return result.result;
		},
	};
	const result = await switchWithOptions(dir, "Empty", { authBackend: locked });
	assert.equal(result.profile, "Empty");
	assert.equal(result.unchanged, false);
	assert.equal(await read(result.backup!), auth("pi-refresh"));
});

test("switch refreshes only Codex in an outgoing legacy snapshot", async (t) => {
	const dir = await tempAgent(t);
	const work = join(dir, "osdy-gentle/profiles/Work/auth.json");
	const canonical = join(dir, "auth.json");
	await writeFile(canonical, auth("work"));
	await saveCurrentAs(dir, "Work");
	const legacy = JSON.parse(auth("legacy"));
	await writeFile(work, JSON.stringify(legacy));
	await writeFile(canonical, auth("work-refreshed"));
	await createProfile(dir, "Home");
	await writeFile(join(dir, "osdy-gentle/profiles/Home/auth.json"), auth("home"));

	assert.deepEqual(await switchAccount(dir, "home"), { profile: "Home", unchanged: false });
	assert.deepEqual(JSON.parse(await read(canonical)), { ...JSON.parse(auth("work-refreshed")), "openai-codex": codex("home") });
	assert.deepEqual(JSON.parse(await read(work)), { ...legacy, "openai-codex": codex("work-refreshed") });
	assert.deepEqual(await readActiveAccount(dir), { status: "valid", profile: "Home" });
	const unchanged = await read(canonical);
	assert.deepEqual(await switchAccount(dir, "Home"), { profile: "Home", unchanged: true });
	assert.equal(await read(canonical), unchanged);
});

test("switching to an empty account removes only live Codex", async (t) => {
	const dir = await tempAgent(t);
	await writeFile(join(dir, "auth.json"), auth("work"));
	await saveCurrentAs(dir, "Work");
	await createProfile(dir, "Fresh");
	assert.deepEqual(await switchAccount(dir, "Fresh"), { profile: "Fresh", unchanged: false });
	assert.deepEqual(JSON.parse(await read(join(dir, "auth.json"))), { anthropic: JSON.parse(auth("work")).anthropic });
	assert.deepEqual(await readActiveAccount(dir), { status: "valid", profile: "Fresh" });
});

test("first switch without an active account backs up unowned credentials", async (t) => {
	const dir = await tempAgent(t);
	await writeFile(join(dir, "auth.json"), auth("orphan"));
	await createProfile(dir, "Home");
	await writeFile(join(dir, "osdy-gentle/profiles/Home/auth.json"), auth("home"));
	const result = await switchAccount(dir, "Home");
	assert.ok(result.backup);
	assert.equal(await read(result.backup!), auth("orphan"));
	assert.deepEqual(JSON.parse(await read(join(dir, "auth.json"))), { ...JSON.parse(auth("orphan")), "openai-codex": codex("home") });
});

test("unknown account switch fails and leaves state unchanged", async (t) => {
	const dir = await tempAgent(t);
	await writeFile(join(dir, "auth.json"), auth("work"));
	await saveCurrentAs(dir, "Work");
	await assert.rejects(switchAccount(dir, "Nope"), /Unknown account: Nope/);
	assert.equal(await read(join(dir, "auth.json")), auth("work"));
	assert.deepEqual(await readActiveAccount(dir), { status: "valid", profile: "Work" });
});

test("rename keeps active pointer; remove refuses the active account", async (t) => {
	const dir = await tempAgent(t);
	await writeFile(join(dir, "auth.json"), auth("work"));
	await saveCurrentAs(dir, "Work");
	await createProfile(dir, "Other");
	assert.equal(await renameProfile(dir, "Work", "Job"), "Job");
	assert.deepEqual(await readActiveAccount(dir), { status: "valid", profile: "Job" });
	await assert.rejects(removeProfile(dir, "Job"), /Cannot remove the active account/);
	await assert.rejects(renameProfile(dir, "Other", "job"), /already exists/);
	await removeProfile(dir, "Other");
	assert.deepEqual(await listProfiles(dir), ["Job"]);
});

test("imports Osdy-Pi accounts once, skipping existing names", async (t) => {
	const dir = await tempAgent(t);
	const osdy = await tempAgent(t);
	for (const [name, tag] of [["Ada", "ada"], ["Work", "osdy-work"]]) {
		await mkdir(join(osdy, "osdy-pi/profiles", name), { recursive: true });
		await writeFile(join(osdy, "osdy-pi/profiles", name, "auth.json"), auth(tag));
	}
	await mkdir(join(osdy, "osdy-pi/profiles/Empty"), { recursive: true });
	await writeFile(join(dir, "auth.json"), auth("work"));
	await saveCurrentAs(dir, "Work");

	const first = await importOsdyProfiles(dir, osdy);
	assert.deepEqual(first, { imported: ["Ada", "Empty"], skipped: ["Work"], failed: [], alreadyImported: false });
	assert.equal(await read(join(dir, "osdy-gentle/profiles/Ada/auth.json")), auth("ada"));
	assert.deepEqual(JSON.parse(await read(join(dir, "osdy-gentle/profiles/Work/auth.json"))), { "openai-codex": codex("work") });
	assert.deepEqual(await readdir(join(dir, "osdy-gentle/profiles/Empty")), []);
	assert.equal(await read(join(osdy, "osdy-pi/profiles/Ada/auth.json")), auth("ada"));

	const second = await importOsdyProfiles(dir, osdy);
	assert.equal(second.alreadyImported, true);
});

test("readStoredCodexCredential exposes only the Codex access token", async (t) => {
	const dir = await tempAgent(t);
	await writeFile(join(dir, "auth.json"), auth("work"));
	await saveCurrentAs(dir, "Work");
	const credential = await readStoredCodexCredential(dir, "work");
	assert.deepEqual(credential, { profile: "Work", access: "a.work.c", expires: 9e15 });
	await createProfile(dir, "Empty");
	await assert.rejects(readStoredCodexCredential(dir, "Empty"), /unavailable/);
});
