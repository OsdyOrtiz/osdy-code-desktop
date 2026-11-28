import assert from "node:assert/strict";
import test from "node:test";
import { registerMcpStatusWidget, type McpStatusHost } from "../src/mcp-status.ts";

const KEY = "osdy-gentle:mcp-status";
type On = NonNullable<McpStatusHost["on"]>;

function fake(api: Omit<McpStatusHost, "on"> = {}) {
	const startHandlers: Parameters<On>[1][] = [];
	const stopHandlers: Parameters<On>[1][] = [];
	const writes: [string, string[] | undefined][] = [];
	const ui = { setWidget: (key: string, lines: string[] | undefined) => { writes.push([key, lines]); } };
	registerMcpStatusWidget({
		...api,
		on(event, handler) {
			if (event === "session_start") startHandlers.push(handler);
			if (event === "session_shutdown") stopHandlers.push(handler);
			return () => {};
		},
	});
	return {
		writes,
		start: (mode = "rpc") => startHandlers.forEach((handler) => handler({}, { mode, ui })),
		stop: () => stopHandlers.forEach((handler) => handler({}, { mode: "rpc", ui })),
	};
}

test("the RPC widget is unavailable if either status method is missing", () => {
	const unexpected = () => assert.fail("a partial API must not be used");
	for (const api of [{}, { getMcpStatus: unexpected }, { onMcpStatusChanged: unexpected }]) {
		const desktop = fake(api);
		desktop.start();
		assert.deepEqual(desktop.writes, [[KEY, ['{"v":1,"available":false}']]]);
	}
});

test("only allow-listed status fields cross the RPC widget", () => {
	const desktop = fake({
		getMcpStatus: () => [
			{ name: "a", state: "ready", tools: 0, url: "https://example.test", since: 129 },
			{ name: "b", state: "error", error: { kind: "auth", message: "private" } },
			{ name: "c", state: "connecting", tools: -1 },
			{ name: "d", state: "disabled", tools: 1.2, error: { kind: "private" } },
			{ name: "e", state: "unknown" }, { name: false, state: "ready" }, null,
		],
		onMcpStatusChanged: () => ({ dispose() {} }),
	});
	desktop.start();
	assert.deepEqual(desktop.writes, [[KEY, [JSON.stringify({
		v: 1, available: true, servers: [
			{ name: "a", state: "ready", tools: 0 },
			{ name: "b", state: "error", error: "auth" },
			{ name: "c", state: "connecting" },
			{ name: "d", state: "disabled" },
		],
	})]]]);
});

test("changes replace the snapshot; restart and shutdown dispose and ignore stale callbacks", () => {
	const listeners: (() => void)[] = [];
	const disposed: number[] = [];
	let snapshot: unknown = [];
	const desktop = fake({
		getMcpStatus: () => snapshot,
		onMcpStatusChanged(callback) {
			const i = listeners.push(callback) - 1;
			return { dispose: () => { disposed.push(i); } };
		},
	});
	desktop.start("interactive");
	assert.deepEqual(desktop.writes, []);
	assert.equal(listeners.length, 0);
	desktop.start();
	assert.deepEqual(desktop.writes, [[KEY, ['{"v":1,"available":true,"servers":[]}']]]);
	snapshot = [{ name: "a", state: "ready", tools: 2 }];
	listeners[0]();
	const ready = [KEY, ['{"v":1,"available":true,"servers":[{"name":"a","state":"ready","tools":2}]}']];
	assert.deepEqual(desktop.writes.at(-1), ready);
	desktop.start();
	assert.deepEqual(disposed, [0]);
	assert.deepEqual(desktop.writes.slice(-2), [[KEY, undefined], ready]);
	const afterRestart = desktop.writes.length;
	listeners[0]();
	assert.equal(desktop.writes.length, afterRestart);
	desktop.stop();
	assert.deepEqual(disposed, [0, 1]);
	assert.deepEqual(desktop.writes.at(-1), [KEY, undefined]);
	listeners[1]();
	assert.equal(desktop.writes.length, afterRestart + 1);
});
