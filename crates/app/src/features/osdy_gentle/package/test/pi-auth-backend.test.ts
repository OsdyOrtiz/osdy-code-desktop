import assert from "node:assert/strict";
import test from "node:test";
import { withPiAuthLock, type PiAuthBackend } from "../src/pi-auth-backend.ts";

const privateError = "private auth backend detail";
const busy = "Pi's auth lock is busy (ELOCKED); no account changes were made.";
const failed = "Pi's auth lock failed before the account operation; no account changes were made.";
const skipped = "Pi's auth lock returned without running the account operation; no account changes were made.";
const after = "Pi's auth lock failed after the account operation. Verify the active account before retrying.";

for (const [name, error, message] of [
	["exact ELOCKED", Object.assign(new Error(privateError), { code: "ELOCKED" }), busy],
	["different code", Object.assign(new Error(privateError), { code: "elocked" }), failed],
	["lookalike message", new Error(`ELOCKED: ${privateError}`), failed],
	["unreadable code", Object.defineProperty(new Error(privateError), "code", { get() { assert.fail("invoked code getter"); } }), failed],
	["proxy error", new Proxy(new Error(privateError), { getOwnPropertyDescriptor() { assert.fail("inspected proxy"); } }), failed],
] as const) {
	test(`Pi lock pre-callback rejection: ${name}`, async () => {
		let attempts = 0;
		let operations = 0;
		const backend: PiAuthBackend = { async withLockAsync() { attempts++; throw error; } };
		await assert.rejects(withPiAuthLock(backend, async () => { operations++; }), { message });
		assert.equal(attempts, 1);
		assert.equal(operations, 0);
	});
}

test("Pi lock returned without the callback", async () => {
	let attempts = 0;
	let operations = 0;
	const backend: PiAuthBackend = { async withLockAsync<T>() { attempts++; return undefined as T; } };
	await assert.rejects(withPiAuthLock(backend, async () => { operations++; }), { message: skipped });
	assert.equal(attempts, 1);
	assert.equal(operations, 0);
});

test("Pi lock post-callback failure is ambiguous even with ELOCKED", async () => {
	let attempts = 0;
	let operations = 0;
	const backend: PiAuthBackend = { async withLockAsync(run) {
		attempts++;
		assert.deepEqual(await run(undefined), { result: "committed" });
		throw Object.assign(new Error(privateError), { code: "ELOCKED" });
	} };
	await assert.rejects(withPiAuthLock(backend, async () => { operations++; return "committed"; }), { message: after });
	assert.equal(attempts, 1);
	assert.equal(operations, 1);
});

test("Pi lock preserves the operation's result or original failure and never issues a backend write", async () => {
	let attempts = 0;
	const backend: PiAuthBackend = { async withLockAsync(run) {
		attempts++;
		const outcome = await run(undefined);
		assert.deepEqual(Object.keys(outcome), ["result"]);
		return outcome.result;
	} };
	assert.equal(await withPiAuthLock(backend, async () => "committed"), "committed");
	const operationError = new Error("operation rollback error");
	await assert.rejects(withPiAuthLock(backend, async () => { throw operationError; }), error => error === operationError);
	assert.equal(attempts, 2);
});
