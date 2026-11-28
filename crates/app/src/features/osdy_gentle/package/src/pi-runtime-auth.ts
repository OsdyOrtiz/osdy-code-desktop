import { basename, isAbsolute, resolve } from "node:path";
import { isProxy } from "node:util/types";
import type { CodexResetActiveAuth } from "./codex-reset.ts";
import type { PiAuthBackend } from "./pi-auth-backend.ts";

export interface PiRuntimeAuthBinding {
	backend: PiAuthBackend;
	readActiveAuth(signal?: AbortSignal): Promise<CodexResetActiveAuth | undefined>;
	withCurrentAuthForDispatch(dispatch: (active: CodexResetActiveAuth) => void, signal?: AbortSignal): Promise<boolean>;
}

const BUNDLED_HOST_CLASSES = { runtime: "_ModelRuntime", store: "_AuthStorage" } as const;
const HOST_CLASSES = {
	"0.99.1": { runtime: "ModelRuntime", store: "AuthStorage" },
	"1.6.0": BUNDLED_HOST_CLASSES,
	"1.6.1": BUNDLED_HOST_CLASSES,
} as const;
type HostClasses = (typeof HOST_CLASSES)[keyof typeof HOST_CLASSES];
const AUTH_TIMEOUT_MS = 1_500;
const CODEX_PROVIDER = "openai-codex";

const AUTH_STAGE = {
	IMPORT: "sdk-import",
	VERSION: "sdk-version",
	EXPECTED_PATH: "expected-path",
	CONTEXT: "context",
	REGISTRY: "registry",
	RUNTIME: "runtime",
	CREDENTIALS: "credentials",
	STORE_SHAPE: "store-shape",
	STORE_PATH: "store-path",
	BACKEND_SHAPE: "backend-shape",
	BACKEND_PATH: "backend-path",
	VERIFIED: "verified",
	EXCEPTION: "inspection-exception",
} as const;
const PATH_CHECK = { UNCHECKED: "unchecked", MATCH: "match", MISMATCH: "mismatch" } as const;
type AuthStage = (typeof AUTH_STAGE)[keyof typeof AUTH_STAGE];
type PathCheck = (typeof PATH_CHECK)[keyof typeof PATH_CHECK];

interface AuthInspection {
	stage: AuthStage;
	storePath: PathCheck;
	backendPath: PathCheck;
}

function hostClasses(version: unknown): HostClasses | undefined {
	if (typeof version === "string" && Object.hasOwn(HOST_CLASSES, version)) {
		return HOST_CLASSES[version as keyof typeof HOST_CLASSES];
	}
}

function isExpectedAuthPath(path: string): boolean {
	return typeof path === "string" && !/[\x00-\x1f\x7f]/.test(path) && isAbsolute(path) &&
		resolve(path) === path && basename(path) === "auth.json";
}

function isInspectable(value: unknown): value is object {
	return typeof value === "object" && value !== null && !isProxy(value);
}

function ownData(value: object, key: string): unknown {
	return Object.getOwnPropertyDescriptor(value, key)?.value;
}

/** Compatibility fingerprint for the non-exported host classes, not an authenticity proof. */
function hostPrototype(instance: object, name: string): object | undefined {
	const prototype: unknown = Object.getPrototypeOf(instance);
	if (!isInspectable(prototype) || Object.getPrototypeOf(prototype) !== Object.prototype) return;
	const constructor = ownData(prototype, "constructor");
	if (typeof constructor !== "function" || isProxy(constructor) || ownData(constructor, "name") !== name ||
		ownData(constructor, "prototype") !== prototype) return;
	return prototype;
}

function hostMethod(instance: object, prototype: object, name: string) {
	if (Object.hasOwn(instance, name)) return;
	const method = ownData(prototype, name);
	if (typeof method === "function" && !isProxy(method)) return method;
}

function hasHostMethods(instance: object, name: string, methods: string[]): boolean {
	const prototype = hostPrototype(instance, name);
	return prototype !== undefined && methods.every(method => hostMethod(instance, prototype, method) !== undefined);
}

function inspectBinding(context: unknown, expectedAuthPath: string, classes: HostClasses, metadata?: AuthInspection) {
	function stop(stage: AuthStage): undefined {
		if (metadata) metadata.stage = stage;
		return undefined;
	}
	if (!isInspectable(context)) return stop(AUTH_STAGE.CONTEXT);
	// The public ExtensionContext exposes modelRegistry as an own getter on this host.
	const field = Object.getOwnPropertyDescriptor(context, "modelRegistry");
	if (!field) return stop(AUTH_STAGE.CONTEXT);
	const registry: unknown = field.get ? field.get.call(context) : field.value;
	if (!isInspectable(registry) || !hasHostMethods(registry, "ModelRegistry", ["getProviderAuth"])) return stop(AUTH_STAGE.REGISTRY);
	const runtime = ownData(registry, "runtime");
	if (!isInspectable(runtime) || !hasHostMethods(runtime, classes.runtime, ["getAuth"])) return stop(AUTH_STAGE.RUNTIME);
	const credentials = ownData(runtime, "credentials");
	if (!isInspectable(credentials)) return stop(AUTH_STAGE.CREDENTIALS);
	const credentialsPrototype = hostPrototype(credentials, "RuntimeCredentials");
	if (!credentialsPrototype) return stop(AUTH_STAGE.CREDENTIALS);
	const hasRuntimeApiKey = hostMethod(credentials, credentialsPrototype, "hasRuntimeApiKey");
	if (!hasRuntimeApiKey || !hostMethod(credentials, credentialsPrototype, "read")) return stop(AUTH_STAGE.CREDENTIALS);
	const store = ownData(credentials, "store");
	if (!isInspectable(store)) return stop(AUTH_STAGE.STORE_SHAPE);
	const storeMatches = ownData(store, "authPath") === expectedAuthPath;
	if (metadata) metadata.storePath = storeMatches ? PATH_CHECK.MATCH : PATH_CHECK.MISMATCH;
	if (!storeMatches) return stop(AUTH_STAGE.STORE_PATH);
	const storePrototype = hostPrototype(store, classes.store);
	if (!storePrototype) return stop(AUTH_STAGE.STORE_SHAPE);
	const readLatestData = hostMethod(store, storePrototype, "readLatestData");
	if (!readLatestData || !["read", "reloadFromStorageAsync", "parseStorageData", "updateReadState"]
		.every(method => hostMethod(store, storePrototype, method))) return stop(AUTH_STAGE.STORE_SHAPE);
	const backend = ownData(store, "storage");
	if (!isInspectable(backend)) return stop(AUTH_STAGE.BACKEND_SHAPE);
	const backendMatches = ownData(backend, "authPath") === expectedAuthPath;
	if (metadata) metadata.backendPath = backendMatches ? PATH_CHECK.MATCH : PATH_CHECK.MISMATCH;
	if (!backendMatches) return stop(AUTH_STAGE.BACKEND_PATH);
	const backendPrototype = hostPrototype(backend, "FileAuthStorageBackend");
	if (!backendPrototype || !["withLock", "ensureParentDir", "ensureFileExists", "acquireLockAsync"]
		.every(method => hostMethod(backend, backendPrototype, method) !== undefined)) return stop(AUTH_STAGE.BACKEND_SHAPE);
	const withLockAsync = hostMethod(backend, backendPrototype, "withLockAsync");
	if (!withLockAsync) return stop(AUTH_STAGE.BACKEND_SHAPE);
	if (metadata) metadata.stage = AUTH_STAGE.VERIFIED;
	return { registry, runtime, backend: backend as PiAuthBackend, withLockAsync, credentials, store, hasRuntimeApiKey, readLatestData };
}

/** Describe the same fingerprint without binding a lock or calling credential methods. */
export function describePiRuntimeAuth(sdk: unknown, context: unknown, expectedPath: () => string): string {
	let version = "unknown";
	const metadata: AuthInspection = { stage: AUTH_STAGE.IMPORT, storePath: PATH_CHECK.UNCHECKED, backendPath: PATH_CHECK.UNCHECKED };
	try {
		if (sdk !== undefined) {
			const value = isInspectable(sdk) ? ownData(sdk, "VERSION") : undefined;
			if (typeof value === "string" && value.length <= 14 && /^(?:0|[1-9]\d{0,3})(?:\.(?:0|[1-9]\d{0,3})){2}$/.test(value)) {
				version = value;
			}
			const classes = hostClasses(value);
			metadata.stage = AUTH_STAGE.VERSION;
			if (classes) {
				const path = expectedPath();
				if (isExpectedAuthPath(path)) inspectBinding(context, path, classes, metadata);
				else metadata.stage = AUTH_STAGE.EXPECTED_PATH;
			}
		}
	} catch {
		metadata.stage = AUTH_STAGE.EXCEPTION;
	}
	return `osdy-auth/v1 sdk=${version} stage=${metadata.stage} storePath=${metadata.storePath} backendPath=${metadata.backendPath}`;
}

function projectOAuth(data: unknown): CodexResetActiveAuth | undefined {
	if (!isInspectable(data) || Array.isArray(data)) return;
	const credential = ownData(data, CODEX_PROVIDER);
	if (!isInspectable(credential) || Array.isArray(credential) || ownData(credential, "type") !== "oauth") return;
	const accessToken = ownData(credential, "access");
	const refresh = ownData(credential, "refresh");
	const expires = ownData(credential, "expires");
	const accountId = ownData(credential, "accountId");
	if (typeof accessToken !== "string" || !/^[\x21-\x7e]+$/.test(accessToken) ||
		typeof refresh !== "string" || !refresh.trim() ||
		typeof expires !== "number" || !Number.isFinite(expires) ||
		(accountId !== undefined && (typeof accountId !== "string" || !/^[\x21-\x7e]+$/.test(accountId)))) return;
	return accountId === undefined ? { accessToken, expires } : { accessToken, expires, accountId };
}

async function withinAuthDeadline<T>(session: AbortSignal | undefined, run: (signal: AbortSignal, live: () => boolean) => Promise<T>): Promise<T | undefined> {
	if (session?.aborted) return;
	const deadlineAt = performance.now() + AUTH_TIMEOUT_MS;
	const timeout = AbortSignal.timeout(AUTH_TIMEOUT_MS);
	const signal = session ? AbortSignal.any([session, timeout]) : timeout;
	const aborted = Promise.withResolvers<undefined>();
	const onAbort = () => aborted.resolve(undefined);
	let open = true;
	const live = () => open && !signal.aborted && performance.now() < deadlineAt;
	signal.addEventListener("abort", onAbort, { once: true });
	try {
		if (!live()) return;
		const result = await Promise.race([run(signal, live), aborted.promise]);
		if (live()) return result;
	} finally {
		open = false;
		signal.removeEventListener("abort", onAbort);
	}
}

function projectSnapshot(current: string | undefined): CodexResetActiveAuth | undefined {
	if (typeof current !== "string") return;
	try { return projectOAuth(JSON.parse(current.replace(/^\uFEFF/, ""))); }
	catch { return undefined; }
}

/** Bind only the characterized host shape and a caller-supplied host agent auth.json path. No I/O at binding time. */
export function resolvePiRuntimeAuth(hostVersion: unknown, context: unknown, expectedAuthPath: string): PiRuntimeAuthBinding | undefined {
	const classes = hostClasses(hostVersion);
	if (!classes || !isExpectedAuthPath(expectedAuthPath)) return;
	try {
		const binding = inspectBinding(context, expectedAuthPath, classes);
		if (!binding) return;
		const { registry, runtime, backend, withLockAsync, credentials, store, hasRuntimeApiKey, readLatestData } = binding;
		const lock = (withLockAsync as PiAuthBackend["withLockAsync"]).bind(backend);
		function canUseOAuth(): boolean {
			const current = inspectBinding(context, expectedAuthPath, classes);
			return current?.backend === backend && current.withLockAsync === withLockAsync &&
				current.registry === registry && current.runtime === runtime &&
				current.credentials === credentials && current.store === store &&
				current.hasRuntimeApiKey === hasRuntimeApiKey && current.readLatestData === readLatestData &&
				hasRuntimeApiKey.call(credentials, CODEX_PROVIDER) === false;
		}
		return {
			backend,
			async readActiveAuth(session) {
				try {
					if (session?.aborted || !canUseOAuth()) return;
					// This non-refreshing host read does not resolve stored API-key commands.
					// The signal also makes reload failures reject instead of using stale data.
					const active = await withinAuthDeadline(session, async (signal, live) => {
						const data: unknown = await readLatestData.call(store, { signal });
						if (live() && canUseOAuth()) return projectOAuth(data);
					});
					if (!session?.aborted) return active;
				} catch { return undefined; }
			},
			async withCurrentAuthForDispatch(dispatch, session) {
				try {
					if (session?.aborted || !canUseOAuth()) return false;
					return (await withinAuthDeadline(session, (signal, live) => lock(async current => {
						// Only the owner snapshot is authoritative here. No recursive host read or await of the POST.
						if (live()) {
							const active = projectSnapshot(current);
							if (active && canUseOAuth() && live()) dispatch(active);
						}
						return { result: true };
					}, { signal }))) === true;
				} catch { return false; }
			},
		};
	} catch { return undefined; }
}
