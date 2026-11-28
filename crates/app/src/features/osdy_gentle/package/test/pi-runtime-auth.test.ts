import assert from "node:assert/strict";
import { resolve } from "node:path";
import test from "node:test";
import { setTimeout as delay } from "node:timers/promises";
import { getEventListeners } from "node:events";
import * as runtimeAuth from "../src/pi-runtime-auth.ts";
const { resolvePiRuntimeAuth } = runtimeAuth;

const path = resolve("synthetic-agent", "auth.json");
const version = "0.99.1";
const provider = "openai-codex";
const oauth = { type: "oauth", access: "fake-access", refresh: "fake-refresh", expires: 1, accountId: "fake-account" };

class FileAuthStorageBackend {
	authPath: string;
	current?: string;
	beforeLock?: (signal: AbortSignal | undefined) => void | Promise<void>;
	inLock = false;
	constructor(authPath = path) { this.authPath = authPath; }
	withLock() { assert.fail("unexpected synchronous lock"); }
	async withLockAsync<T>(run: (current: string | undefined) => Promise<{ result: T }>, options?: { signal?: AbortSignal }) {
		assert.equal(this.authPath, path);
		assert.equal(this.inLock, false, "do not reacquire the owner lock");
		await this.beforeLock?.(options?.signal);
		this.inLock = true;
		try { return (await run(this.current)).result; } finally { this.inLock = false; }
	}
	ensureParentDir() { assert.fail("unexpected directory setup"); }
	ensureFileExists() { assert.fail("unexpected file setup"); }
	acquireLockAsync() { assert.fail("unexpected filesystem lock"); }
}
class AuthStorage {
	data: unknown = { [provider]: oauth };
	reads = 0;
	beforeRead?: (signal: AbortSignal) => void | Promise<void>;
	storage: unknown;
	authPath: string;
	constructor(storage: unknown, authPath = path) {
		this.storage = storage;
		this.authPath = authPath;
	}
	async readLatestData(options?: { signal?: AbortSignal }) {
		assert.ok(options?.signal instanceof AbortSignal, "host must propagate errors instead of using stale data");
		this.reads++;
		await this.beforeRead?.(options.signal);
		return this.data;
	}
	read() { assert.fail("host read can execute configured API-key commands"); }
	reloadFromStorageAsync() { assert.fail("unexpected host reload"); }
	parseStorageData() { assert.fail("unexpected host parse"); }
	updateReadState() { assert.fail("unexpected host update"); }
}
class RuntimeCredentials {
	overrides = new Map<string, string>();
	store: unknown;
	constructor(store: unknown) { this.store = store; }
	hasRuntimeApiKey(id: string) {
		assert.equal(id, provider);
		return this.overrides.has(id);
	}
	read() { assert.fail("overlay read can reach configured API-key commands"); }
}
class ModelRuntime {
	credentials: unknown;
	constructor(credentials: unknown) { this.credentials = credentials; }
	getAuth() { assert.fail("unexpected runtime provider auth"); }
}
class ModelRegistry {
	runtime: unknown;
	constructor(runtime: unknown) { this.runtime = runtime; }
	getProviderAuth() { assert.fail("unexpected refreshing provider auth"); }
}
function fixture(backend = new FileAuthStorageBackend(), storagePath = path) {
	const store = new AuthStorage(backend, storagePath);
	const credentials = new RuntimeCredentials(store);
	const registry = new ModelRegistry(new ModelRuntime(credentials));
	const ctx = { get modelRegistry() { return registry; } };
	return { ctx, registry, credentials, store, backend };
}

test("auth status uses the production fingerprint without acquiring or reading credentials", () => {
	const status = runtimeAuth.describePiRuntimeAuth;
	assert.equal(typeof status, "function");
	const { ctx, store, backend, credentials, registry } = fixture();
	backend.beforeLock = () => assert.fail("status attempted the Pi lock");
	store.beforeRead = () => assert.fail("status attempted a host auth read");
	const other = resolve("private-other-agent", "auth.json");
	const check = (stage: string, storePath = "unchecked", backendPath = "unchecked", input: unknown = ctx, expected = path) => {
		assert.equal(status({ VERSION: version }, input, () => expected),
			`osdy-auth/v1 sdk=0.99.1 stage=${stage} storePath=${storePath} backendPath=${backendPath}`);
		assert.equal(resolvePiRuntimeAuth(version, input, expected)?.backend, stage === "verified" ? backend : undefined);
	};
	check("verified", "match", "match");
	check("expected-path", "unchecked", "unchecked", ctx, `${path}\nprivate`);
	check("context", "unchecked", "unchecked", {});
	check("registry", "unchecked", "unchecked", { modelRegistry: {} });
	check("inspection-exception", "unchecked", "unchecked", { get modelRegistry() { throw new Error(`private ${other}`); } });
	backend.authPath = other;
	check("backend-path", "match", "mismatch");
	Object.defineProperty(backend, "authPath", { get() { assert.fail("status evaluated a backend path getter"); }, configurable: true });
	check("backend-path", "match", "mismatch");
	Object.defineProperty(backend, "authPath", { value: path, configurable: true });
	Object.defineProperty(backend, "withLockAsync", { value: () => assert.fail("unexpected shadow lock") });
	check("backend-shape", "match", "match");
	store.authPath = `${path}/../auth.json`;
	check("store-path", "mismatch");
	store.authPath = path;
	Object.defineProperty(store, "readLatestData", { value: () => assert.fail("unexpected shadow read") });
	check("store-shape", "match");
	credentials.store = new Proxy(store, { getOwnPropertyDescriptor() { assert.fail("status inspected a proxy"); } });
	check("store-shape");
	const runtime = registry.runtime as ModelRuntime;
	runtime.credentials = {};
	check("credentials");
	registry.runtime = {};
	check("runtime");
	assert.equal(store.reads, 0);
	assert.equal(backend.inLock, false);
	assert.deepEqual(store.data, { [provider]: oauth });
});

test("auth status only prints bounded numeric SDK versions, never an unsupported host's data", () => {
	const status = runtimeAuth.describePiRuntimeAuth;
	assert.equal(typeof status, "function");
	const { ctx, backend } = fixture();
	const unexpectedPath = () => assert.fail("unsupported SDK must not resolve the agent path");
	const suspect = { get modelRegistry() { assert.fail("unsupported SDK must not inspect context"); } };
	const suffix = "storePath=unchecked backendPath=unchecked";
	assert.equal(status(undefined, suspect, unexpectedPath), `osdy-auth/v1 sdk=unknown stage=sdk-import ${suffix}`);
	for (const VERSION of [undefined, 123, "0.99.1\n", "0.99.1 private", "v0.99.1", "00.99.1", "12345.1.2", "0.99.1-beta", { toString() { assert.fail("version was coerced"); } }]) {
		assert.equal(status({ VERSION }, suspect, unexpectedPath), `osdy-auth/v1 sdk=unknown stage=sdk-version ${suffix}`);
	}
	assert.equal(status({ VERSION: "1.6.99" }, suspect, unexpectedPath), `osdy-auth/v1 sdk=1.6.99 stage=sdk-version ${suffix}`);
	assert.equal(status({ VERSION: version }, suspect, () => { throw new Error("private directory error"); }),
		`osdy-auth/v1 sdk=0.99.1 stage=inspection-exception ${suffix}`);
	assert.equal(resolvePiRuntimeAuth(version, ctx, path)?.backend, backend);
});

test("binds the session's actual lock owner and projects only current Codex OAuth fields", async () => {
	const { ctx, store, backend } = fixture();
	const binding = resolvePiRuntimeAuth(version, ctx, path);
	assert.ok(binding);
	assert.equal(binding.backend, backend);
	assert.equal(await binding.backend.withLockAsync(async () => ({ result: "locked" })), "locked");
	assert.equal(store.reads, 0);
	store.data = { [provider]: oauth, anthropic: { type: "oauth", access: "unrelated" } };
	assert.deepEqual(await binding.readActiveAuth(), { accessToken: "fake-access", expires: 1, accountId: "fake-account" });
	store.data = { [provider]: { ...oauth, access: "new-access", accountId: undefined } };
	assert.deepEqual(await binding.readActiveAuth(), { accessToken: "new-access", expires: 1 });
	assert.equal(store.reads, 2);
});

function bundledFixture() {
	const { backend, credentials } = fixture();
	const BundledRuntime = class _ModelRuntime { credentials = credentials; getAuth() {} };
	const BundledStore = class _AuthStorage {
		storage = backend;
		authPath = path;
		reads = 0;
		async readLatestData() { this.reads++; return { [provider]: oauth }; }
		read() {}
		reloadFromStorageAsync() {}
		parseStorageData() {}
		updateReadState() {}
	};
	const store = new BundledStore();
	credentials.store = store;
	const ctx = { modelRegistry: new ModelRegistry(new BundledRuntime()) };
	return { ctx, backend, store };
}

test("preserves the exact Pi 1.6.0 and 0.99.1 fingerprints", () => {
	const { ctx, backend } = bundledFixture();
	assert.equal(resolvePiRuntimeAuth("1.6.0", ctx, path)?.backend, backend);
	assert.equal(runtimeAuth.describePiRuntimeAuth({ VERSION: "1.6.0" }, ctx, () => path),
		"osdy-auth/v1 sdk=1.6.0 stage=verified storePath=match backendPath=match");
	assert.equal(resolvePiRuntimeAuth("0.99.1", ctx, path), undefined);
});

test("recognizes the exact Pi 1.6.1 bundle without relaxing store and backend checks", () => {
	const { ctx, backend, store } = bundledFixture();
	const sdk = { VERSION: "1.6.1" };
	const snapshot = JSON.stringify({ [provider]: oauth, anthropic: { type: "oauth", access: "unrelated" } });
	backend.current = snapshot;
	let locks = 0;
	backend.beforeLock = () => { locks++; };
	const check = (diagnostic: string, owner?: FileAuthStorageBackend) => {
		assert.equal(runtimeAuth.describePiRuntimeAuth(sdk, ctx, () => path), `osdy-auth/v1 sdk=1.6.1 ${diagnostic}`);
		assert.equal(resolvePiRuntimeAuth(sdk.VERSION, ctx, path)?.backend, owner);
	};
	check("stage=verified storePath=match backendPath=match", backend);
	store.authPath = resolve("other-agent", "auth.json");
	check("stage=store-path storePath=mismatch backendPath=unchecked");
	store.authPath = path;
	backend.authPath = resolve("other-agent", "auth.json");
	check("stage=backend-path storePath=match backendPath=mismatch");
	backend.authPath = path;
	Object.defineProperty(backend, "withLockAsync", { value: backend.withLockAsync });
	check("stage=backend-shape storePath=match backendPath=match");
	assert.equal(store.reads, 0);
	assert.equal(locks, 0);
	assert.equal(backend.current, snapshot);
	assert.equal(resolvePiRuntimeAuth(sdk.VERSION, fixture().ctx, path), undefined);
});

test("rejects Pi 1.6.2 and inherited version keys before inspecting the bundled owner", () => {
	const { ctx } = bundledFixture();
	const unexpectedPath = () => assert.fail("unsupported SDK must not resolve the agent path");
	for (const VERSION of ["1.6.2", "constructor", "toString", "__proto__"]) {
		const sdk = VERSION === "1.6.2" ? VERSION : "unknown";
		assert.equal(runtimeAuth.describePiRuntimeAuth({ VERSION }, ctx, unexpectedPath),
			`osdy-auth/v1 sdk=${sdk} stage=sdk-version storePath=unchecked backendPath=unchecked`);
		assert.equal(resolvePiRuntimeAuth(VERSION, ctx, path), undefined);
	}
});

test("requires the exact SDK version and both matching canonical credential paths", () => {
	const { ctx } = fixture();
	for (const unknownVersion of [undefined, "0.99", "0.99.2", "0.99.1-dev", "v0.99.1"]) {
		assert.equal(resolvePiRuntimeAuth(unknownVersion, ctx, path), undefined);
	}
	const other = resolve("other-agent", "auth.json");
	assert.equal(resolvePiRuntimeAuth(version, ctx, other), undefined);
	assert.equal(resolvePiRuntimeAuth(version, fixture(new FileAuthStorageBackend(other)).ctx, path), undefined);
	assert.equal(resolvePiRuntimeAuth(version, fixture(new FileAuthStorageBackend(), other).ctx, path), undefined);
	for (const unsafe of ["auth.json", resolve("synthetic-agent") + "/../synthetic-agent/auth.json", resolve("synthetic-agent", "auth.json\0"), resolve("synthetic-agent", "other.json")]) {
		assert.equal(resolvePiRuntimeAuth(version, fixture(new FileAuthStorageBackend(unsafe), unsafe).ctx, unsafe), undefined);
	}
});

test("rejects missing, custom, in-memory, read-only and shadowed host shapes", () => {
	const backend = new FileAuthStorageBackend();
	const wrap = (store: unknown) => ({ modelRegistry: new ModelRegistry(new ModelRuntime(new RuntimeCredentials(store))) });
	class InMemoryAuthStorageBackend extends FileAuthStorageBackend {}
	class ReadOnlyAuthStorage extends AuthStorage {}
	const shadowed = Object.assign(new FileAuthStorageBackend(), { withLockAsync: backend.withLockAsync });
	for (const ctx of [null, {}, { modelRegistry: { runtime: new ModelRuntime(new RuntimeCredentials(new AuthStorage(backend))) } },
		{ get modelRegistry() { throw new Error("private context error"); } },
		wrap(new AuthStorage(new InMemoryAuthStorageBackend())), wrap(new ReadOnlyAuthStorage(backend)),
		wrap(new AuthStorage(shadowed)), wrap(new AuthStorage({ authPath: path, withLockAsync: backend.withLockAsync })),
		wrap(new AuthStorage(new Proxy(backend, { getPrototypeOf() { assert.fail("unexpected proxy trap"); } })))]) {
		assert.equal(resolvePiRuntimeAuth(version, ctx, path), undefined);
	}
});

test("runtime API-key overrides win even when empty or installed during a read", async () => {
	const { ctx, store, credentials } = fixture();
	const binding = resolvePiRuntimeAuth(version, ctx, path);
	assert.ok(binding);
	credentials.overrides.set(provider, "");
	assert.equal(await binding.readActiveAuth(), undefined);
	assert.equal(store.reads, 0);
	credentials.overrides.delete(provider);
	store.beforeRead = () => { credentials.overrides.set(provider, "fake-runtime-key"); };
	assert.equal(await binding.readActiveAuth(), undefined);
	assert.equal(store.reads, 1);
});

test("a non-cooperative owner read is bounded and its late completion cannot return OAuth", { timeout: 4_000 }, async () => {
	const { ctx, store } = fixture();
	const binding = resolvePiRuntimeAuth(version, ctx, path)!;
	const watchdog = new AbortController();
	const late = Promise.withResolvers<void>();
	const expired = Symbol("owner read waited beyond its deadline");
	let ownerSignal: AbortSignal | undefined;
	store.beforeRead = signal => { ownerSignal = signal; return late.promise; };
	const reading = binding.readActiveAuth();
	try {
		assert.equal(await Promise.race([reading, delay(2_500, expired, { signal: watchdog.signal })]), undefined);
		assert.equal(ownerSignal?.aborted, true);
	} finally {
		watchdog.abort();
		late.resolve();
		await reading;
	}
	assert.equal(await reading, undefined);
	assert.equal(store.reads, 1);
	assert.deepEqual(store.data, { [provider]: oauth });
});

test("session cancellation settles a non-cooperative read and prevents late OAuth", async () => {
	const { ctx, store } = fixture();
	const binding = resolvePiRuntimeAuth(version, ctx, path)!;
	const session = new AbortController();
	const late = Promise.withResolvers<void>();
	let signalAtRead: AbortSignal | undefined;
	store.beforeRead = signal => { signalAtRead = signal; return late.promise; };
	const reading = binding.readActiveAuth(session.signal);
	session.abort();
	const stillWaiting = Symbol("read did not settle when cancelled");
	try { assert.equal(await Promise.race([reading, delay(100, stillWaiting)]), undefined); }
	finally { late.resolve(); await reading; }
	assert.equal(await reading, undefined);
	assert.equal(signalAtRead?.aborted, true);
	assert.deepEqual(getEventListeners(signalAtRead!, "abort"), []);
	assert.equal(store.reads, 1);
});

test("dispatch projects only the original owner's locked snapshot, not its cached auth", async () => {
	const { ctx, backend, credentials, store, registry } = fixture();
	const binding = resolvePiRuntimeAuth(version, ctx, path)!;
	backend.current = `\uFEFF${JSON.stringify({ [provider]: { ...oauth, access: "new-access" }, anthropic: oauth })}`;
	const seen: unknown[] = [];
	const dispatch = (active: unknown) => { assert.equal(backend.inLock, true); seen.push(active); };
	await binding.withCurrentAuthForDispatch(dispatch);
	assert.equal(backend.inLock, false);
	assert.deepEqual(seen, [{ accessToken: "new-access", expires: 1, accountId: "fake-account" }]);
	for (const snapshot of [undefined, "{", "[]", JSON.stringify({ [provider]: { ...oauth, refresh: "" } })]) {
		backend.current = snapshot;
		await binding.withCurrentAuthForDispatch(dispatch);
	}
	backend.current = JSON.stringify({ [provider]: oauth });
	backend.beforeLock = () => { credentials.overrides.set(provider, ""); };
	await binding.withCurrentAuthForDispatch(dispatch);
	credentials.overrides.clear();
	backend.beforeLock = () => { registry.runtime = new ModelRuntime(new RuntimeCredentials(store)); };
	await binding.withCurrentAuthForDispatch(dispatch);
	assert.equal(seen.length, 1);
	assert.equal(store.reads, 0);
});

test("a non-cooperative owner lock cannot dispatch a callback delivered after timeout or cancellation", { timeout: 4_000 }, async () => {
	for (const cancel of [true, false]) {
		const { ctx, backend, store } = fixture();
		const binding = resolvePiRuntimeAuth(version, ctx, path)!;
		backend.current = JSON.stringify({ [provider]: oauth });
		const session = new AbortController();
		const late = Promise.withResolvers<void>();
		const watchdog = new AbortController();
		const tooLate = Symbol("lock did not settle");
		let ownerSignal: AbortSignal | undefined;
		let operations = 0;
		let unlock: Promise<void> | undefined;
		backend.beforeLock = signal => { ownerSignal = signal; return (unlock = late.promise); };
		const locked = binding.withCurrentAuthForDispatch(() => { operations++; }, session.signal);
		if (cancel) session.abort();
		try {
			assert.equal(await Promise.race([locked, delay(cancel ? 100 : 2_500, tooLate, { signal: watchdog.signal })]), false);
			assert.equal(ownerSignal?.aborted, true);
			assert.deepEqual(getEventListeners(ownerSignal!, "abort"), []);
		} finally {
			watchdog.abort();
			late.resolve();
			await unlock;
			await locked;
		}
		assert.equal(backend.inLock, false);
		assert.equal(operations, 0);
		assert.equal(store.reads, 0);
	}
});

test("missing, non-OAuth and malformed credentials or rejected host reads return no active auth", async () => {
	const { ctx, store } = fixture();
	const binding = resolvePiRuntimeAuth(version, ctx, path);
	assert.ok(binding);
	for (const credential of [undefined, null, { type: "api_key", key: "!never-run" }, { ...oauth, access: "a\nb" },
		{ ...oauth, refresh: undefined }, { ...oauth, refresh: "" }, { ...oauth, expires: Infinity }, { ...oauth, accountId: 1 },
		{ ...oauth, get access() { assert.fail("unexpected credential getter"); } }]) {
		store.data = { [provider]: credential };
		assert.equal(await binding.readActiveAuth(), undefined);
	}
	store.beforeRead = () => { throw new Error("private host error"); };
	assert.equal(await binding.readActiveAuth(), undefined);
});
