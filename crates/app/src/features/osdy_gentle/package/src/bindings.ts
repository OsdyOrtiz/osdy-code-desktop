/**
 * Osdy profile = Codex account + optional Gentle model profile.
 * Stored in `<agentDir>/osdy-gentle/bindings.json`:
 *   { "version": 1, "bindings": { "<account>": { "gentleProfile": "<name>" } } }
 */
import { t } from "./i18n.ts";
import { readFile, rename, writeFile, unlink } from "node:fs/promises";
import { randomUUID } from "node:crypto";
import { join, resolve } from "node:path";
import { STORE_DIR, checkManagedDirectory, ensurePrivateDirectory, isProfileName } from "./account-store.ts";

export type Bindings = Record<string, { gentleProfile: string }>;

function bindingsDir(agentDir: string): string {
	return join(resolve(agentDir), STORE_DIR);
}

function key(account: string): string {
	return account.toLowerCase();
}

export async function readBindings(agentDir: string): Promise<Bindings> {
	const directory = bindingsDir(agentDir);
	if (!(await checkManagedDirectory(directory))) return {};
	try {
		const parsed = JSON.parse(await readFile(join(directory, "bindings.json"), "utf8"));
		if (!parsed || parsed.version !== 1 || typeof parsed.bindings !== "object" || !parsed.bindings) return {};
		const result: Bindings = {};
		for (const [account, value] of Object.entries(parsed.bindings as Record<string, unknown>)) {
			const profile = (value as { gentleProfile?: unknown } | null)?.gentleProfile;
			if (isProfileName(account) && typeof profile === "string" && profile.length > 0 && profile.length <= 128)
				result[key(account)] = { gentleProfile: profile };
		}
		return result;
	} catch {
		return {};
	}
}

export function bindingFor(bindings: Bindings, account: string): string | undefined {
	return bindings[key(account)]?.gentleProfile;
}

/** Bind (or with `undefined`, unbind) a Gentle profile to an account. */
export async function setBinding(agentDir: string, account: string, gentleProfile: string | undefined): Promise<void> {
	if (!isProfileName(account)) throw new Error(t("Invalid account name: {account}", { account }));
	const bindings = await readBindings(agentDir);
	if (gentleProfile === undefined) delete bindings[key(account)];
	else bindings[key(account)] = { gentleProfile };
	const directory = bindingsDir(agentDir);
	await ensurePrivateDirectory(directory);
	const path = join(directory, "bindings.json");
	const tmp = `${path}.${process.pid}.${randomUUID()}.tmp`;
	try {
		await writeFile(tmp, `${JSON.stringify({ version: 1, bindings }, null, 2)}\n`, { mode: 0o600, flag: "wx" });
		await rename(tmp, path);
	} catch (error) {
		await unlink(tmp).catch(() => {});
		throw error;
	}
}

/** Keep a binding attached when its account is renamed; drop it when the account is removed. */
export async function moveBinding(agentDir: string, from: string, to: string | undefined): Promise<void> {
	const bindings = await readBindings(agentDir);
	const current = bindingFor(bindings, from);
	if (current === undefined) return;
	await setBinding(agentDir, from, undefined);
	if (to !== undefined) await setBinding(agentDir, to, current);
}
