import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { bindingFor, moveBinding, readBindings, setBinding } from "../src/bindings.ts";
import {
	PROFILES_SERVICE_CHANNEL,
	PROFILES_SERVICE_SCHEMA,
	applyGentleProfile,
	discoverProfilesService,
	listGentleProfiles,
	resolveGentleConfigHome,
	type ApplyResult,
	type ProfilesService,
} from "../src/gentle-profiles.ts";

test("bindings round-trip case-insensitively and follow renames/removals", async () => {
	const dir = await mkdtemp(join(tmpdir(), "osdy-gentle-b-"));
	assert.deepEqual(await readBindings(dir), {});
	await setBinding(dir, "Work", "balanced");
	assert.equal(bindingFor(await readBindings(dir), "work"), "balanced");
	await moveBinding(dir, "Work", "Job");
	assert.equal(bindingFor(await readBindings(dir), "Work"), undefined);
	assert.equal(bindingFor(await readBindings(dir), "Job"), "balanced");
	await moveBinding(dir, "Job", undefined);
	assert.deepEqual(await readBindings(dir), {});
	await assert.rejects(setBinding(dir, "../x", "p"), /Invalid account name/);
	await rm(dir, { recursive: true });
});

test("resolves Gentle config home with gentle-pi precedence", () => {
	assert.equal(resolveGentleConfigHome({ GENTLE_PI_CONFIG_HOME: "/a", GENTLE_SHELL_CONFIG_HOME: "/b" }, "/h"), "/a");
	assert.equal(resolveGentleConfigHome({ GENTLE_SHELL_CONFIG_HOME: "/b" }, "/h"), "/b");
	assert.equal(resolveGentleConfigHome({ GENTLE_SHELL_CONFIG_HOME: "rel" }, "/h"), "/h/.pi/gentle-ai");
});

test("lists Gentle profile names from profiles.json", async () => {
	const dir = await mkdtemp(join(tmpdir(), "osdy-gentle-p-"));
	assert.deepEqual(await listGentleProfiles(dir), []);
	await writeFile(join(dir, "profiles.json"), JSON.stringify({ kind: "gentle-pi.agent_model_profiles", version: 1, profiles: { fast: {}, balanced: {} } }));
	assert.deepEqual(await listGentleProfiles(dir), ["balanced", "fast"]);
	await rm(dir, { recursive: true });
});

function fakeService(results: ApplyResult[], calls: unknown[]): ProfilesService {
	return {
		schema: PROFILES_SERVICE_SCHEMA,
		apply: async (request) => {
			calls.push(request);
			return results.shift()!;
		},
		dispose: () => {},
	};
}

test("discovers the profiles owner over the event bus, or times out", async () => {
	const service = fakeService([], []);
	const ctx = { id: "ctx" };
	const found = await discoverProfilesService({
		emit: (channel, data) => {
			assert.equal(channel, PROFILES_SERVICE_CHANNEL);
			const { context, respond } = data as { context: unknown; respond: (v: unknown) => void };
			assert.equal(context, ctx);
			respond({ schema: "other" });
			respond(service);
		},
	}, ctx);
	assert.equal(found, service);
	assert.equal(await discoverProfilesService({ emit: () => {} }, ctx, 5), undefined);
});

test("apply resends with the token only after confirmation", async () => {
	const calls: unknown[] = [];
	const confirmation: ApplyResult = { status: "confirmation_required", title: "T", message: "M", confirmation: "tok" };
	const applied: ApplyResult = { status: "applied", notes: [{ level: "info", text: "ok" }] };
	let shown: string[] = [];
	const accepted = await applyGentleProfile(fakeService([confirmation, applied], calls), "fast", async (t, m) => { shown = [t, m]; return true; });
	assert.deepEqual(accepted, applied);
	assert.deepEqual(shown, ["T", "M"]);
	assert.deepEqual(calls, [{ profile: "fast", mode: "session" }, { profile: "fast", mode: "session", confirmation: "tok" }]);

	const declinedCalls: unknown[] = [];
	const declined = await applyGentleProfile(fakeService([confirmation], declinedCalls), "fast", async () => false);
	assert.deepEqual(declined, { status: "cancelled" });
	assert.equal(declinedCalls.length, 1);
});
