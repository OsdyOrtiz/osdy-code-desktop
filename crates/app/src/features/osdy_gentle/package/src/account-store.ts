/**
 * Codex account snapshots for Gentle Shell, ported from Osdy-Pi's
 * `scripts/osdy-pi-account-profiles.mjs`.
 *
 * Layout under the Pi agent dir (Gentle Shell: `~/.gentle-shell/agent`):
 *   auth.json                                 canonical credentials Pi reads
 *   osdy-gentle/active-account.json           { version: 1, profile }
 *   osdy-gentle/profiles/<name>/auth.json     Codex snapshot (also reads legacy whole auth)
 *   osdy-gentle/backups/auth-<ts>.json        unowned canonical auth before a switch
 *
 * Switches share an account fence with other store mutations and use Pi's auth
 * lock for the keyed OAuth update and any rollback.
 */
import { constants as fsConstants, lstatSync, unlinkSync, type Stats } from "node:fs";
import {
	chmod,
	lstat,
	mkdir,
	open,
	readdir,
	rename,
	rmdir,
	unlink,
	writeFile,
} from "node:fs/promises";
import { randomUUID } from "node:crypto";
import { homedir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { PI_AUTH_BACKEND_REQUIRED, withPiAuthLock, type PiAuthBackend } from "./pi-auth-backend.ts";

const PROFILE_NAME = /^[A-Za-z0-9](?:[A-Za-z0-9-]{0,62})$/;
const RESERVED = new Set(["default", "profiles", "auth.json", "backups"]);
const MAX_AUTH_BYTES = 4 * 1024 * 1024;
const MAX_METADATA_BYTES = 4096;
const LOCK_TIMEOUT_MS = 2000;
const LOCK_RETRY_MS = 25;
const CODEX = "openai-codex";
export const STORE_DIR = "osdy-gentle";
export const ACCOUNT_SWITCH_BUSY_MESSAGE = "The agent is busy; nothing was switched. Try again when it is idle.";
const ACTIVE_FILE = "active-account.json";
const SWITCH_LOCK = "account-switch.lock";
const IMPORT_MARKER = "imported-from-osdy.json";

let serial: Promise<unknown> = Promise.resolve();

export type ActiveAccount =
	| { status: "valid"; profile: string }
	| { status: "unset" }
	| { status: "invalid" };

export type StoredCodexCredential = {
	profile: string;
	access: string;
	expires: number;
	accountId?: string;
};

export type ImportResult = {
	imported: string[];
	skipped: string[];
	failed: string[];
	alreadyImported: boolean;
};

export function isProfileName(value: unknown): value is string {
	return (
		typeof value === "string" &&
		PROFILE_NAME.test(value) &&
		!RESERVED.has(value.toLowerCase())
	);
}

export function validateProfileName(value: unknown): string {
	if (!isProfileName(value))
		throw new Error(
			"Profile names must use ASCII letters, numbers, and hyphens (not reserved names).",
		);
	return value;
}

/** Osdy-Pi's shared agent dir, the source of the one-time import. */
export function defaultOsdySharedDir(env: NodeJS.ProcessEnv = process.env): string {
	return resolve(env.OSDY_PI_SHARED_AGENT_DIR ?? join(homedir(), ".pi", "agent"));
}

function storeDir(agentDir: string): string {
	return join(resolve(agentDir), STORE_DIR);
}

function profilesDir(agentDir: string): string {
	return join(storeDir(agentDir), "profiles");
}

function canonicalAuthPath(agentDir: string): string {
	return join(resolve(agentDir), "auth.json");
}

function isErrorCode(error: unknown, code: string): boolean {
	return !!error && typeof error === "object" && "code" in error && (error as { code: unknown }).code === code;
}

function boundedError(error: unknown): string {
	return error instanceof Error ? error.message.slice(0, 180) : "unknown error";
}

class ManagedDirectoryError extends Error {}

async function statIfPresent(path: string): Promise<Stats | undefined> {
	try {
		return await lstat(path);
	} catch (error) {
		if (isErrorCode(error, "ENOENT")) return undefined;
		throw error;
	}
}

/** Check each managed directory from its verified parent, never following its final component. */
export async function checkManagedDirectory(path: string): Promise<Stats | undefined> {
	const stat = await statIfPresent(path);
	if (stat && !stat.isDirectory()) throw new ManagedDirectoryError(`Managed path is not a directory: ${path}`);
	return stat;
}

async function ensureManagedDirectory(path: string): Promise<Stats> {
	let stat = await checkManagedDirectory(path);
	if (!stat) {
		try {
			await mkdir(path, { mode: 0o700, recursive: true });
		} catch (error) {
			if (!isErrorCode(error, "EEXIST")) throw error;
		}
		stat = await checkManagedDirectory(path);
		if (!stat) throw new Error(`Managed directory disappeared: ${path}`);
	}
	return stat;
}

export async function ensurePrivateDirectory(path: string): Promise<void> {
	const stat = await ensureManagedDirectory(path);
	if ((stat.mode & 0o7777) !== 0o700) await chmod(path, 0o700);
}

async function replacePrivateFile(path: string, contents: string | Buffer): Promise<void> {
	const temporary = join(dirname(path), `.${basename(path)}.${process.pid}.${randomUUID()}.tmp`);
	try {
		await writeFile(temporary, contents, { mode: 0o600, flag: "wx" });
		await chmod(temporary, 0o600);
		await rename(temporary, path);
	} catch (error) {
		await unlink(temporary).catch(() => {});
		throw error;
	}
}

/** Read a bounded regular file without following symlinks; undefined when missing. */
async function readBoundedFile(path: string, maxBytes: number, before?: Stats): Promise<Buffer | undefined> {
	before ??= await statIfPresent(path);
	if (!before) return undefined;
	if (!before.isFile() || before.size > maxBytes)
		throw new Error("Account file is not a bounded regular file.");
	let handle;
	try {
		handle = await open(path, fsConstants.O_RDONLY | (fsConstants.O_NOFOLLOW ?? 0));
	} catch (error) {
		if (isErrorCode(error, "ELOOP")) throw new Error("Account file is not a bounded regular file.");
		if (isErrorCode(error, "ENOENT")) return undefined;
		throw error;
	}
	try {
		const opened = await handle.stat();
		if (opened.dev !== before.dev || opened.ino !== before.ino || !opened.isFile() || opened.size > maxBytes)
			throw new Error("Account file changed while reading it.");
		const contents = Buffer.alloc(opened.size);
		const { bytesRead } = await handle.read(contents, 0, contents.length, 0);
		if (bytesRead !== opened.size) throw new Error("Account file changed while reading it.");
		return contents;
	} finally {
		await handle.close();
	}
}

function releaseOwnedLock(path: string, identity: Stats): void {
	try {
		const current = lstatSync(path);
		if (current.dev === identity.dev && current.ino === identity.ino) unlinkSync(path);
	} catch {
		// Failure to release leaves the fence closed, not open to overlapping owners.
	}
}

async function withLock<T>(agentDir: string, operation: () => Promise<T>, deferStoreMode: boolean): Promise<T> {
	const directory = storeDir(agentDir);
	if (deferStoreMode) await ensureManagedDirectory(directory);
	else await ensurePrivateDirectory(directory);
	const lockPath = join(directory, SWITCH_LOCK);
	const deadline = Date.now() + LOCK_TIMEOUT_MS;
	let handle;
	for (;;) {
		await checkManagedDirectory(directory);
		try {
			handle = await open(lockPath, "wx", 0o600);
			break;
		} catch (error) {
			if (!isErrorCode(error, "EEXIST")) throw error;
			if (Date.now() >= deadline) throw new Error("Timed out waiting for account switch lock.");
			// Reclaiming by path can displace a fresh owner. Fail closed after a crash.
			await delay(LOCK_RETRY_MS);
		}
	}
	let identity: Stats | undefined;
	try {
		identity = await handle.stat();
		await handle.writeFile(`${JSON.stringify({ pid: process.pid })}\n`);
		return await operation();
	} finally {
		if (identity && await checkManagedDirectory(directory).catch(() => undefined)) releaseOwnedLock(lockPath, identity);
		await handle.close().catch(() => {});
	}
}

/** Serialize in-process, then across processes with the lock file. */
function serialized<T>(agentDir: string, operation: () => Promise<T>, { deferStoreMode = false } = {}): Promise<T> {
	const next = serial.then(() => withLock(agentDir, operation, deferStoreMode));
	serial = next.catch(() => {});
	return next;
}

/** Run a reset or other active-account operation under the same fence as switching. */
export function withAccountSwitchFence<T>(agentDir: string, run: () => Promise<T>): Promise<T> {
	return serialized(agentDir, run);
}

async function findProfile(agentDir: string, name: string): Promise<string | undefined> {
	const matches = (await listProfiles(agentDir)).filter((p) => p.toLowerCase() === name.toLowerCase());
	if (matches.length > 1) throw new Error("Profile lookup is ambiguous due to a case-insensitive collision.");
	return matches[0];
}

async function requireProfile(agentDir: string, name: string): Promise<string> {
	const found = await findProfile(agentDir, validateProfileName(name));
	if (found === undefined) throw new Error(`Unknown account: ${name}`);
	return found;
}

function profileAuthPath(agentDir: string, name: string): string {
	return join(profilesDir(agentDir), name, "auth.json");
}

export async function listProfiles(agentDir: string): Promise<string[]> {
	const directory = profilesDir(agentDir);
	if (!(await checkManagedDirectory(storeDir(agentDir))) || !(await checkManagedDirectory(directory))) return [];
	try {
		const entries = await readdir(directory, { withFileTypes: true });
		const profiles: string[] = [];
		for (const entry of entries) {
			if (isProfileName(entry.name) && (entry.isDirectory() || entry.isSymbolicLink()) &&
				await checkManagedDirectory(join(directory, entry.name))) profiles.push(entry.name);
		}
		return profiles.sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
	} catch (error) {
		if (isErrorCode(error, "ENOENT")) return [];
		throw error;
	}
}

export async function readActiveAccount(agentDir: string): Promise<ActiveAccount> {
	try {
		if (!(await checkManagedDirectory(storeDir(agentDir)))) return { status: "unset" };
		const raw = await readBoundedFile(join(storeDir(agentDir), ACTIVE_FILE), MAX_METADATA_BYTES);
		if (raw === undefined) return { status: "unset" };
		const parsed = JSON.parse(raw.toString("utf8"));
		if (!parsed || parsed.version !== 1 || !isProfileName(parsed.profile)) return { status: "invalid" };
		const profile = await findProfile(agentDir, parsed.profile);
		return profile === undefined ? { status: "invalid" } : { status: "valid", profile };
	} catch (error) {
		if (error instanceof ManagedDirectoryError) throw error;
		return { status: "invalid" };
	}
}

async function writeActive(agentDir: string, name: string): Promise<void> {
	await checkManagedDirectory(storeDir(agentDir));
	await replacePrivateFile(
		join(storeDir(agentDir), ACTIVE_FILE),
		`${JSON.stringify({ version: 1, profile: name })}\n`,
	);
}

async function createProfileDir(agentDir: string, name: string): Promise<string> {
	const safe = validateProfileName(name);
	if ((await findProfile(agentDir, safe)) !== undefined)
		throw new Error(`An account named ${safe} already exists.`);
	await ensurePrivateDirectory(profilesDir(agentDir));
	await mkdir(join(profilesDir(agentDir), safe), { mode: 0o700 });
	return safe;
}

/** Create an empty account; switching to it removes live Codex so the user can sign in. */
export async function createProfile(agentDir: string, name: string): Promise<string> {
	return serialized(agentDir, () => createProfileDir(agentDir, name));
}

export type AccountAuthOptions = { authBackend?: PiAuthBackend };
type AuthDocument = Record<string, unknown>;

function isAuthDocument(value: unknown): value is AuthDocument {
	return value !== null && typeof value === "object" && !Array.isArray(value);
}

function parseAuthDocument(raw: Buffer | undefined, label: string): AuthDocument {
	if (raw === undefined) return {};
	try {
		const value: unknown = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(raw));
		if (isAuthDocument(value)) return value;
	} catch {
		// Parser errors can include credential bytes; never surface them.
	}
	throw new Error(`${label} is not a valid auth document.`);
}

function codexSignIn(auth: AuthDocument, label: string): AuthDocument | undefined {
	if (!Object.hasOwn(auth, CODEX)) return undefined;
	const codex = auth[CODEX];
	if (!isAuthDocument(codex) || codex.type !== "oauth" ||
		typeof codex.access !== "string" || !codex.access ||
		typeof codex.refresh !== "string" || !codex.refresh ||
		typeof codex.expires !== "number" || !Number.isFinite(codex.expires) ||
		("accountId" in codex && (typeof codex.accountId !== "string" || !codex.accountId))) {
		throw new Error(`${label} contains invalid Codex credentials.`);
	}
	return codex;
}

function encodeAuth(auth: AuthDocument): string {
	const contents = `${JSON.stringify(auth)}\n`;
	if (Buffer.byteLength(contents) > MAX_AUTH_BYTES) throw new Error("Updated auth document exceeds the size limit.");
	return contents;
}

function updateCodex(auth: AuthDocument, codex: AuthDocument | undefined): string {
	if (codex === undefined) delete auth[CODEX];
	else auth[CODEX] = codex;
	return encodeAuth(auth);
}

async function withCanonicalAuth<T>(path: string, backend: PiAuthBackend, run: () => Promise<T>): Promise<T> {
	// Pi may read the file as it acquires its lock. Reject unsafe file types
	// and sizes first, then do the authoritative bounded read inside the lock.
	const current = await statIfPresent(path);
	if (current && (!current.isFile() || current.size > MAX_AUTH_BYTES))
		throw new Error("Account file is not a bounded regular file.");
	return withPiAuthLock(backend, run);
}

/** Save only the current Codex OAuth sign-in and mark the new account active. */
export async function saveCurrentAs(agentDir: string, name: string, { authBackend }: AccountAuthOptions = {}): Promise<string> {
	if (!authBackend) throw new Error(PI_AUTH_BACKEND_REQUIRED);
	const canonical = canonicalAuthPath(agentDir);
	return serialized(agentDir, () => withCanonicalAuth(canonical, authBackend, async () => {
		const label = "Current auth.json";
		const current = parseAuthDocument(await readBoundedFile(canonical, MAX_AUTH_BYTES), label);
		const codex = codexSignIn(current, label);
		if (!codex) throw new Error("There is no signed-in Codex account to save.");
		const contents = encodeAuth({ [CODEX]: codex });
		const safe = await createProfileDir(agentDir, name);
		const snapshot = profileAuthPath(agentDir, safe);
		let written = false;
		try {
			await replacePrivateFile(snapshot, contents);
			written = true;
			await writeActive(agentDir, safe);
		} catch (error) {
			try {
				if (written) await unlink(snapshot);
				await rmdir(dirname(snapshot));
			} catch {
				throw new Error("Account save failed and rollback could not be completed. Verify the account before retrying.");
			}
			throw error;
		}
		return safe;
	}));
}

export type SwitchResult = { profile: string; backup?: string; unchanged: boolean };

async function restoreAuth(path: string, previous: Buffer | undefined): Promise<void> {
	if (previous !== undefined) await replacePrivateFile(path, previous);
	else await unlink(path);
}

export async function switchAccount(
	agentDir: string,
	name: string,
	{ guard, authBackend }: AccountAuthOptions & { guard?: () => boolean } = {},
): Promise<SwitchResult> {
	if (!authBackend) throw new Error(PI_AUTH_BACKEND_REQUIRED);
	return serialized(agentDir, async () => {
		const target = await requireProfile(agentDir, name);
		const canonical = canonicalAuthPath(agentDir);
		return withCanonicalAuth(canonical, authBackend, async () => {
			const active = await readActiveAccount(agentDir);
			const canonicalBefore = await readBoundedFile(canonical, MAX_AUTH_BYTES);
			const current = parseAuthDocument(canonicalBefore, "Current auth.json");
			const currentCodex = codexSignIn(current, "Current auth.json");
			const incoming = parseAuthDocument(await readBoundedFile(profileAuthPath(agentDir, target), MAX_AUTH_BYTES), "Incoming account auth.json");
			const incomingCodex = codexSignIn(incoming, "Incoming account auth.json");
			const unchanged = active.status === "valid" && active.profile === target;
			const canonicalNext = !unchanged && (currentCodex || incomingCodex) ? updateCodex(current, incomingCodex) : undefined;
			let outgoing: { path: string; previous: Buffer | undefined; next: string } | undefined;
			if (!unchanged && active.status === "valid") {
				const path = profileAuthPath(agentDir, active.profile);
				const previous = await readBoundedFile(path, MAX_AUTH_BYTES);
				const snapshot = parseAuthDocument(previous, "Outgoing account auth.json");
				if (currentCodex || Object.hasOwn(snapshot, CODEX))
					outgoing = { path, previous, next: updateCodex(snapshot, currentCodex) };
			}
			if (guard) {
				if (!guard()) throw new Error(ACCOUNT_SWITCH_BUSY_MESSAGE);
				await ensurePrivateDirectory(storeDir(agentDir));
			}
			if (unchanged) {
				await writeActive(agentDir, target);
				return { profile: target, unchanged: true };
			}
			let backup: string | undefined;
			if (canonicalBefore !== undefined && active.status !== "valid") {
				const backups = join(storeDir(agentDir), "backups");
				await ensurePrivateDirectory(backups);
				backup = join(backups, `auth-${new Date().toISOString().replace(/[:.]/g, "-")}.json`);
				await replacePrivateFile(backup, canonicalBefore);
			}
			let outgoingWritten = false;
			let canonicalWritten = false;
			try {
				if (outgoing) {
					await replacePrivateFile(outgoing.path, outgoing.next);
					outgoingWritten = true;
				}
				if (canonicalNext !== undefined) {
					await replacePrivateFile(canonical, canonicalNext);
					canonicalWritten = true;
				}
				await writeActive(agentDir, target);
			} catch (error) {
				const restores: Promise<void>[] = [];
				if (canonicalWritten) restores.push(restoreAuth(canonical, canonicalBefore));
				if (outgoingWritten && outgoing) restores.push(restoreAuth(outgoing.path, outgoing.previous));
				if ((await Promise.allSettled(restores)).some((result) => result.status === "rejected")) {
					throw new Error("Account switch failed and rollback could not be completed. Verify the active account before retrying.");
				}
				if (!canonicalWritten && !outgoingWritten) throw error;
				throw new Error(`Account switch failed and was rolled back: ${boundedError(error)}`);
			}
			return backup === undefined ? { profile: target, unchanged: false } : { profile: target, backup, unchanged: false };
		});
	}, { deferStoreMode: guard !== undefined });
}

export async function renameProfile(agentDir: string, oldName: string, newName: string): Promise<string> {
	return serialized(agentDir, async () => {
		const from = await requireProfile(agentDir, oldName);
		const to = validateProfileName(newName);
		const existing = await findProfile(agentDir, to);
		if (existing !== undefined && existing.toLowerCase() !== from.toLowerCase())
			throw new Error(`An account named ${to} already exists.`);
		const active = await readActiveAccount(agentDir);
		const tmp = join(profilesDir(agentDir), `.rename.${process.pid}.${randomUUID()}.tmp`);
		await rename(join(profilesDir(agentDir), from), tmp);
		await rename(tmp, join(profilesDir(agentDir), to));
		if (active.status === "valid" && active.profile === from) await writeActive(agentDir, to);
		return to;
	});
}

export async function removeProfile(agentDir: string, name: string): Promise<void> {
	return serialized(agentDir, async () => {
		const profile = await requireProfile(agentDir, name);
		const active = await readActiveAccount(agentDir);
		if (active.status === "valid" && active.profile === profile)
			throw new Error("Cannot remove the active account. Switch to another account first.");
		const dir = join(profilesDir(agentDir), profile);
		const entries = await readdir(dir, { withFileTypes: true });
		for (const entry of entries) {
			const stat = await lstat(join(dir, entry.name));
			if (stat.isDirectory()) throw new Error(`Refusing to remove unexpected directory in account: ${entry.name}`);
		}
		for (const entry of entries) await unlink(join(dir, entry.name));
		await rmdir(dir);
	});
}

/**
 * One-time import of Osdy-Pi accounts (`<osdyDir>/osdy-pi/profiles/<name>/auth.json`).
 * Existing names are skipped, never overwritten. A marker records the import.
 */
export async function importOsdyProfiles(
	agentDir: string,
	osdySharedDir: string = defaultOsdySharedDir(),
	{ force = false }: { force?: boolean } = {},
): Promise<ImportResult> {
	return serialized(agentDir, async () => {
		const marker = join(storeDir(agentDir), IMPORT_MARKER);
		const result: ImportResult = { imported: [], skipped: [], failed: [], alreadyImported: false };
		if (!force && (await readBoundedFile(marker, MAX_METADATA_BYTES).catch(() => undefined)) !== undefined) {
			result.alreadyImported = true;
			return result;
		}
		const sourceRoot = join(resolve(osdySharedDir), "osdy-pi", "profiles");
		let entries;
		try {
			entries = await readdir(sourceRoot, { withFileTypes: true });
		} catch (error) {
			if (isErrorCode(error, "ENOENT")) throw new Error(`No Osdy-Pi accounts found in ${sourceRoot}`);
			throw error;
		}
		for (const entry of entries.filter((e) => e.isDirectory() && isProfileName(e.name)).sort((a, b) => (a.name < b.name ? -1 : 1))) {
			const name = entry.name;
			if ((await findProfile(agentDir, name)) !== undefined) {
				result.skipped.push(name);
				continue;
			}
			try {
				const auth = await readBoundedFile(join(sourceRoot, name, "auth.json"), MAX_AUTH_BYTES);
				await createProfileDir(agentDir, name);
				if (auth !== undefined) await replacePrivateFile(profileAuthPath(agentDir, name), auth);
				result.imported.push(name);
			} catch (error) {
				if (error instanceof ManagedDirectoryError) throw error;
				result.failed.push(name);
			}
		}
		await checkManagedDirectory(storeDir(agentDir));
		await replacePrivateFile(
			marker,
			`${JSON.stringify({ version: 1, source: sourceRoot, at: new Date().toISOString(), imported: result.imported })}\n`,
		);
		return result;
	});
}

/** Read only a stored account's Codex OAuth access token; never canonical auth or refresh tokens. */
export async function readStoredCodexCredential(agentDir: string, name: string): Promise<StoredCodexCredential> {
	try {
		const profile = await requireProfile(agentDir, name);
		const raw = await readBoundedFile(profileAuthPath(agentDir, profile), MAX_AUTH_BYTES);
		if (raw === undefined) throw new Error("missing");
		const parsed = JSON.parse(raw.toString("utf8"));
		const c = parsed && !Array.isArray(parsed) ? parsed["openai-codex"] : undefined;
		if (
			!c || typeof c !== "object" || c.type !== "oauth" ||
			typeof c.access !== "string" || !c.access ||
			typeof c.expires !== "number" || !Number.isFinite(c.expires) ||
			("accountId" in c && (typeof c.accountId !== "string" || !c.accountId))
		)
			throw new Error("unsupported");
		return { profile, access: c.access, expires: c.expires, ...(c.accountId ? { accountId: c.accountId } : {}) };
	} catch (error) {
		if (error instanceof ManagedDirectoryError) throw error;
		throw new Error("Stored account credentials are unavailable.");
	}
}
