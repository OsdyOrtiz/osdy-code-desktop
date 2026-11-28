import { isProxy } from "node:util/types";

/** Narrow structural interface to Pi's FileAuthStorageBackend for this agent's auth.json. */
export interface PiAuthBackend {
	withLockAsync<T>(fn: (current: string | undefined) => Promise<PiAuthLockResult<T>>, options?: PiAuthLockOptions): Promise<T>;
}

interface PiAuthLockOptions {
	signal?: AbortSignal;
}

interface PiAuthLockResult<T> {
	result: T;
}

export const PI_AUTH_BACKEND_REQUIRED = "Pi's auth lock is unavailable; no account changes were made.";

function isLockBusy(error: unknown): boolean {
	// Do not call accessors or proxy traps on a host error.
	return typeof error === "object" && error !== null && !isProxy(error) &&
		Object.getOwnPropertyDescriptor(error, "code")?.value === "ELOCKED";
}

/**
 * Use Pi's lock as a fence. The operation makes its own bounded reads and atomic
 * writes (including rollback) inside the callback. Never return `next`: Pi would
 * write it after the callback, outside the operation's rollback boundary.
 */
export async function withPiAuthLock<T>(backend: PiAuthBackend, run: () => Promise<T>): Promise<T> {
	let operationStarted = false;
	const operationFailure = Symbol();
	let failure: unknown;
	let result: T;
	try {
		result = await backend.withLockAsync(async () => {
			operationStarted = true;
			try {
				return { result: await run() };
			} catch (error) {
				failure = error;
				throw operationFailure;
			}
		});
	} catch (error) {
		if (error === operationFailure) throw failure;
		// The host's error may contain auth file data. Do not expose it or claim rollback.
		if (operationStarted) {
			throw new Error("Pi's auth lock failed after the account operation. Verify the active account before retrying.");
		}
		throw new Error(isLockBusy(error)
			? "Pi's auth lock is busy (ELOCKED); no account changes were made."
			: "Pi's auth lock failed before the account operation; no account changes were made.");
	}
	if (!operationStarted) {
		throw new Error("Pi's auth lock returned without running the account operation; no account changes were made.");
	}
	return result;
}
