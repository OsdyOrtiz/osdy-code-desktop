import { test } from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createCodexResetStore } from "../src/codex-reset-store.ts";

const accountId = "mock-account";
const attempt = { creditId: " exact/+é ", requestId: "11111111-1111-4111-8111-111111111111", expiresAt: 2_000 };
const other = { ...attempt, requestId: "22222222-2222-4222-8222-222222222222" };
const unavailable = { message: "Codex reset journal is unavailable" };

function inProcess(agentDir: string, program: string) {
	const moduleUrl = new URL("../src/codex-reset-store.ts", import.meta.url).href;
	const child = spawn(process.execPath, ["--input-type=module", "--eval", `
		import { createCodexResetStore } from ${JSON.stringify(moduleUrl)};
		const store = createCodexResetStore(${JSON.stringify(agentDir)});
		const accountId = ${JSON.stringify(accountId)};
		const attempt = ${JSON.stringify(attempt)};
		${program}
	`], { stdio: ["ignore", "pipe", "pipe"] });
	let stdout = "";
	let stderr = "";
	child.stdout.on("data", value => { stdout += String(value); });
	child.stderr.on("data", value => { stderr += String(value); });
	return once(child, "close").then(([code]) => ({ code, stdout, stderr }));
}

test("read-only check doesn't create a journal; pending cannot be overwritten and confirmed keeps the server result", async t => {
	const agentDir = await mkdtemp(join(tmpdir(), "osdy-reset-store-"));
	t.after(() => rm(agentDir, { recursive: true, force: true }));
	const store = createCodexResetStore(agentDir);
	assert.equal(await store.read(accountId), undefined);
	assert.deepEqual(await readdir(agentDir), []);
	const lease = await store.acquire(accountId);
	try {
		await lease.start(attempt);
		await assert.rejects(lease.start(other), unavailable);
		await assert.rejects(lease.confirm(other, { code: "reset", windowsReset: 2 }), unavailable);
		assert.deepEqual(await store.read(accountId), { status: "pending", ...attempt });
		assert.equal(await store.read("different-account"), undefined);
		await lease.confirm(attempt, { code: "reset", windowsReset: 2 });
		await lease.start(other);
		await lease.discard(other);
	} finally { await lease.release(); }
	assert.deepEqual(await createCodexResetStore(agentDir).read(accountId), { status: "confirmed", ...attempt, code: "reset", windowsReset: 2 });
});

test("a second process cannot claim a live owner, and a dead owner leaves a readable, fail-closed pending journal", async t => {
	const agentDir = await mkdtemp(join(tmpdir(), "osdy-reset-process-"));
	t.after(() => rm(agentDir, { recursive: true, force: true }));
	const store = createCodexResetStore(agentDir);
	const lease = await store.acquire(accountId);
	const probe = `try { await store.acquire(accountId); console.log("unexpected lock"); } catch (e) { console.log(e.message); }`;
	try {
		assert.deepEqual(await inProcess(agentDir, probe), { code: 0, stdout: `${unavailable.message}\n`, stderr: "" });
	} finally { await lease.release(); }
	assert.deepEqual(await inProcess(agentDir, `const lease = await store.acquire(accountId); await lease.start(attempt); process.exit(0);`), { code: 0, stdout: "", stderr: "" });
	assert.deepEqual(await store.read(accountId), { status: "pending", ...attempt });
	await assert.rejects(store.acquire(accountId), unavailable);
	const otherLease = await store.acquire("different-account");
	await otherLease.release();
});
