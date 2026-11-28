import { createHash, randomUUID } from "node:crypto";
import { constants, type Stats } from "node:fs";
import { lstat, mkdir, open, rename, unlink, type FileHandle } from "node:fs/promises";
import { join, resolve } from "node:path";
import { isResetCreditId, isResetOutcome, isResetRequestId, type CodexResetOutcome, type CodexResetRequest } from "./codex-reset-transport.ts";

const STATUS = { PENDING: "pending", CONFIRMED: "confirmed" } as const;
export interface CodexResetAttempt extends CodexResetRequest { expiresAt: number }
export type CodexResetRecord = CodexResetAttempt & (
	{ status: typeof STATUS.PENDING } | ({ status: typeof STATUS.CONFIRMED } & CodexResetOutcome)
);
export interface CodexResetLease {
	read(): Promise<CodexResetRecord | undefined>;
	start(attempt: CodexResetAttempt): Promise<void>;
	confirm(attempt: CodexResetAttempt, outcome: CodexResetOutcome): Promise<void>;
	/** Only the owner that can prove this request was never dispatched may discard it. */
	discard(attempt: CodexResetAttempt): Promise<void>;
	release(): Promise<void>;
}
export interface CodexResetStore {
	read(accountId: string): Promise<CodexResetRecord | undefined>;
	acquire(accountId: string): Promise<CodexResetLease>;
}

const MAX_BYTES = 8_192;
const unavailable = () => new Error("Codex reset journal is unavailable");
const hasCode = (error: unknown, code: string) => !!error && typeof error === "object" && "code" in error && error.code === code;
const sameFile = (a: Stats, b: Stats) => a.dev === b.dev && a.ino === b.ino;

function checkedAttempt(attempt: CodexResetAttempt): CodexResetAttempt {
	const { creditId, requestId, expiresAt } = attempt;
	if (!isResetCreditId(creditId) || !isResetRequestId(requestId) || !Number.isSafeInteger(expiresAt) || expiresAt < 0) throw unavailable();
	return { creditId, requestId, expiresAt };
}

function decodeRecord(text: string, accountId: string): CodexResetRecord {
	const raw = JSON.parse(text) as Record<string, unknown>;
	if (!raw || typeof raw !== "object" || Array.isArray(raw) || raw.version !== 1 || raw.accountId !== accountId) throw unavailable();
	const attempt = checkedAttempt(raw as unknown as CodexResetAttempt);
	const keys = ["version", "accountId", "status", "creditId", "requestId", "expiresAt"];
	if (raw.status === STATUS.CONFIRMED) keys.push("code", "windowsReset");
	if (Object.keys(raw).some(key => !keys.includes(key))) throw unavailable();
	if (raw.status === STATUS.PENDING) return { ...attempt, status: STATUS.PENDING };
	if (raw.status === STATUS.CONFIRMED && isResetOutcome(raw.code, raw.windowsReset))
		return { ...attempt, status: STATUS.CONFIRMED, code: raw.code, windowsReset: raw.windowsReset } as CodexResetRecord;
	throw unavailable();
}

async function checkDir(path: string, make: boolean, privateMode = false): Promise<Stats | undefined> {
	if (make) {
		try { await mkdir(path, { mode: 0o700 }); }
		catch (error) { if (!hasCode(error, "EEXIST")) throw error; }
	}
	try {
		const stat = await lstat(path);
		if (!stat.isDirectory() || (privateMode && (stat.mode & 0o7777) !== 0o700)) throw unavailable();
		return stat;
	} catch (error) { if (hasCode(error, "ENOENT") && !make) return undefined; throw error; }
}

async function syncDirectory(path: string): Promise<void> {
	const directory = await open(path, constants.O_RDONLY | constants.O_DIRECTORY | constants.O_NOFOLLOW);
	try { await directory.sync(); } finally { await directory.close(); }
}

/** The agent directory and its ancestors are trusted. Managed child directories may not be symlinks. */
export function createCodexResetStore(agentDir: string): CodexResetStore {
	const base = resolve(agentDir);
	const managed = join(base, "osdy-gentle");
	const root = join(managed, "codex-reset-attempts");

	function accountPaths(accountId: string) {
		if (typeof accountId !== "string" || !/^[A-Za-z0-9_-]{1,512}$/.test(accountId)) throw unavailable();
		const key = createHash("sha256").update(accountId).digest("hex");
		return { journal: join(root, `${key}.json`), lock: join(root, `${key}.lock`) };
	}

	async function rootInfo(make = false) {
		if (!(await checkDir(managed, make))) return undefined;
		return checkDir(root, make, true);
	}

	async function readJournal(path: string, accountId: string): Promise<CodexResetRecord | undefined> {
		let file: FileHandle;
		try { file = await open(path, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK); }
		catch (error) { if (hasCode(error, "ENOENT")) return undefined; throw error; }
		try {
			const stat = await file.stat();
			if (!stat.isFile() || (stat.mode & 0o7777) !== 0o600 || stat.size > MAX_BYTES) throw unavailable();
			const buffer = Buffer.alloc(MAX_BYTES + 1);
			let offset = 0;
			while (offset < buffer.length) {
				const { bytesRead } = await file.read(buffer, offset, buffer.length - offset, offset);
				if (!bytesRead) break;
				offset += bytesRead;
			}
			if (offset > MAX_BYTES) throw unavailable();
			return decodeRecord(new TextDecoder("utf-8", { fatal: true }).decode(buffer.subarray(0, offset)), accountId);
		} finally { await file.close(); }
	}

	return {
		async read(accountId) {
			try {
				const { journal } = accountPaths(accountId);
				if (!(await rootInfo())) return undefined;
				return await readJournal(journal, accountId);
			} catch { throw unavailable(); }
		},
		async acquire(accountId) {
			let handle: FileHandle | undefined;
			try {
				const { journal, lock } = accountPaths(accountId);
				const rootStat = await rootInfo(true);
				if (!rootStat) throw unavailable();
				// Never reclaim an uncertain owner. A read-only check still works after a crash.
				const held = await open(lock, constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL | constants.O_NOFOLLOW, 0o600);
				handle = held;
				const identity = await held.stat();
				let released = false;
				let poisoned = false;
				let started: { requestId: string; previous: CodexResetRecord | undefined } | undefined;
				let serial: Promise<unknown> = Promise.resolve();
				function safely<T>(run: () => Promise<T>): Promise<T> {
					const next = serial.then(run).catch(() => { throw unavailable(); });
					serial = next.catch(() => {});
					return next;
				}
				async function checkLease() {
					if (released || poisoned) throw unavailable();
					const currentRoot = await rootInfo();
					if (!currentRoot || !sameFile(currentRoot, rootStat!)) throw unavailable();
					const current = await lstat(lock);
					if (!current.isFile() || (current.mode & 0o7777) !== 0o600 || !sameFile(current, identity)) throw unavailable();
				}
				async function read() {
					await checkLease();
					return readJournal(journal, accountId);
				}
				async function atomicSave(record: CodexResetRecord) {
					const temp = join(root, `${randomUUID()}.tmp`);
					let created = false;
					try {
						await checkLease();
						const file = await open(temp, constants.O_WRONLY | constants.O_CREAT | constants.O_EXCL | constants.O_NOFOLLOW, 0o600);
						created = true;
						try {
							await file.writeFile(JSON.stringify({ version: 1, accountId, ...record }), "utf8");
							await file.sync();
						} finally { await file.close(); }
						await checkLease();
						await rename(temp, journal);
						created = false;
						try { await syncDirectory(root); }
						catch (error) { poisoned = true; throw error; }
					} finally { if (created) await unlink(temp); }
				}
				async function requirePending(attempt: CodexResetAttempt) {
					const { requestId, creditId, expiresAt } = checkedAttempt(attempt);
					const previous = await read();
					if (started?.requestId !== requestId || previous?.status !== STATUS.PENDING || previous.requestId !== requestId ||
						previous.creditId !== creditId || previous.expiresAt !== expiresAt) throw unavailable();
					return previous;
				}
				return {
					read: () => safely(read),
					start: attempt => safely(async () => {
						const next = checkedAttempt(attempt);
						const previous = await read();
						if (previous?.status === STATUS.PENDING || previous?.requestId === next.requestId) throw unavailable();
						// Persist the directory entries too, including on the first attempt.
						await syncDirectory(base);
						await syncDirectory(managed);
						await atomicSave({ ...next, status: STATUS.PENDING });
						started = { requestId: next.requestId, previous };
					}),
					confirm: (attempt, outcome) => safely(async () => {
						const { code, windowsReset } = outcome;
						if (!isResetOutcome(code, windowsReset)) throw unavailable();
						const previous = await requirePending(attempt);
						await atomicSave({ ...previous, status: STATUS.CONFIRMED, code, windowsReset });
						started = undefined;
					}),
					discard: attempt => safely(async () => {
						await requirePending(attempt);
						if (started?.previous) await atomicSave(started.previous);
						else {
							await checkLease();
							await unlink(journal);
							try { await syncDirectory(root); }
							catch (error) { poisoned = true; throw error; }
						}
						started = undefined;
					}),
					release: () => safely(async () => {
						try {
							await checkLease();
							await unlink(lock);
						} finally {
							released = true;
							await held.close();
						}
					}),
				};
			} catch { await handle?.close().catch(() => {}); throw unavailable(); }
		},
	};
}
