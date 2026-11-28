import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, type TestContext } from "node:test";
import {
	createProfile, importOsdyProfiles, readActiveAccount, removeProfile,
	renameProfile, saveCurrentAs, switchAccount,
} from "../src/account-store.ts";

import type { PiAuthBackend } from "../src/pi-auth-backend.ts";

const AUTH = "auth.json";
const authBackend: PiAuthBackend = { async withLockAsync(run) { return (await run(undefined)).result; } };
const workAuth = '{"openai-codex":{"type":"oauth","access":"work","refresh":"work","expires":1},"otherProvider":{}}\n';

async function scratch(t: TestContext) {
	const root = await mkdtemp(join(tmpdir(), "osdy-r2-"));
	t.after(() => rm(root, { recursive: true, force: true }));
	return root;
}

async function hashes(...paths: string[]) {
	return Promise.all(paths.map(async (path) =>
		createHash("sha256").update(await readFile(path)).digest("hex")));
}

test("S4/S5: a bad incoming snapshot preserves canonical and active bytes", async (t) => {
	const agent = await scratch(t);
	const store = join(agent, "osdy-gentle");
	const canonical = join(agent, AUTH);
	const active = join(store, "active-account.json");
	const outgoing = join(store, "profiles", "Work", AUTH);
	await writeFile(canonical, workAuth);
	await saveCurrentAs(agent, "Work", { authBackend });
	await createProfile(agent, "Home");
	const incoming = join(store, "profiles", "Home", AUTH);
	await writeFile(canonical, workAuth.replaceAll("work", "refreshed"));
	const before = await hashes(canonical, active, outgoing);
	for (const [document, message] of [
		['{"malformed":"synthetic"', "Incoming account auth.json is not a valid auth document."],
		['[]', "Incoming account auth.json is not a valid auth document."],
		['{"openai-codex":null}', "Incoming account auth.json contains invalid Codex credentials."],
	]) {
		await writeFile(incoming, document);
		await assert.rejects(switchAccount(agent, "Home", { guard: () => true, authBackend }), { message });
		assert.deepEqual(await hashes(canonical, active, outgoing), before);
		assert.equal(await readFile(incoming, "utf8"), document);
		assert.deepEqual(await readActiveAccount(agent), { status: "valid", profile: "Work" });
	}
});

test("S5: malformed current auth rejects a switch without mutating either snapshot or the active account", async (t) => {
	const agent = await scratch(t);
	const store = join(agent, "osdy-gentle");
	const canonical = join(agent, AUTH);
	await writeFile(canonical, workAuth);
	await saveCurrentAs(agent, "Work", { authBackend });
	await createProfile(agent, "Home");
	const incoming = join(store, "profiles", "Home", AUTH);
	await writeFile(incoming, workAuth.replaceAll("work", "home"));
	await writeFile(canonical, '"bad document"');
	const paths = [canonical, join(store, "active-account.json"), join(store, "profiles", "Work", AUTH), incoming];
	const before = await hashes(...paths);
	await assert.rejects(switchAccount(agent, "Home", { authBackend }), { message: "Current auth.json is not a valid auth document." });
	assert.deepEqual(await hashes(...paths), before);
	assert.deepEqual(await readActiveAccount(agent), { status: "valid", profile: "Work" });
});

test("S5: failed active metadata commit rolls back live auth while retaining the original backup", async (t) => {
	const agent = await scratch(t);
	const store = join(agent, "osdy-gentle");
	const canonical = join(agent, AUTH);
	await writeFile(canonical, workAuth);
	await createProfile(agent, "Home");
	await mkdir(join(store, "active-account.json"));
	await assert.rejects(switchAccount(agent, "Home", { authBackend }), /Account switch failed and was rolled back:/);
	assert.equal(await readFile(canonical, "utf8"), workAuth);
	assert.deepEqual(await readActiveAccount(agent), { status: "invalid" });
	const backups = join(store, "backups");
	const names = await readdir(backups);
	assert.equal(names.length, 1);
	assert.equal(await readFile(join(backups, names[0]!), "utf8"), workAuth);
});

test("S5/S6: import then rename/remove leaves the Osdy source intact", async (t) => {
	const agent = await scratch(t);
	const osdy = await scratch(t);
	const sourceDir = join(osdy, "osdy-pi", "profiles", "Work");
	await mkdir(sourceDir, { recursive: true });
	const source = join(sourceDir, AUTH);
	await writeFile(source, '{ "synthetic": "work", "otherProvider": {} }\n');
	const before = await hashes(source);

	assert.deepEqual(await importOsdyProfiles(agent, osdy), {
		imported: ["Work"], skipped: [], failed: [], alreadyImported: false,
	});
	assert.equal(await renameProfile(agent, "Work", "Job"), "Job");
	assert.deepEqual(await hashes(join(agent, "osdy-gentle", "profiles", "Job", AUTH)), before);
	await removeProfile(agent, "Job");
	assert.deepEqual(await hashes(source), before);
	assert.deepEqual(await importOsdyProfiles(agent, osdy), {
		imported: [], skipped: [], failed: [], alreadyImported: true,
	});
});
