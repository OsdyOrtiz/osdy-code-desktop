import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { createProfile } from "../src/account-store.ts";
import { osdyAccountCommand } from "../src/commands.ts";

const sha = async (path: string) => createHash("sha256").update(await readFile(path)).digest("hex");

test("an owned error after a dialog speaks the language the command started in", async (t) => {
	const root = await mkdtemp(join(tmpdir(), "osdy-message-locale-"));
	const previous = process.env.OSDY_DESKTOP_LOCALE_FILE;
	t.after(async () => {
		if (previous === undefined) delete process.env.OSDY_DESKTOP_LOCALE_FILE;
		else process.env.OSDY_DESKTOP_LOCALE_FILE = previous;
		await rm(root, { recursive: true, force: true });
	});
	const locale = join(root, "agent-locale");
	process.env.OSDY_DESKTOP_LOCALE_FILE = locale;
	await writeFile(locale, "es");
	await createProfile(root, "Keep-Me");
	const marker = join(root, "osdy-gentle", "active-account.json");
	await writeFile(marker, '{"version":1,"profile":"Keep-Me"}\n');
	const before = await sha(marker);
	const dialogs: string[][] = [];
	const notices: string[][] = [];
	await osdyAccountCommand("", { isIdle: () => true, ui: {
		async select(title: string, options: string[]) { return options[title === "Cuentas Osdy" ? 7 : 0]; },
		async confirm(title: string, text: string) {
			dialogs.push([title, text]);
			await writeFile(locale, "en"); // the app switches language while the dialog is open
			return true;
		},
		input: async () => assert.fail("no input expected"),
		notify: (text: string, level?: string) => notices.push([text, level ?? ""]),
	} }, { agentDir: root, events: { emit() {} } });
	assert.equal(await sha(marker), before, "the active account marker is untouched");
	assert.deepEqual(dialogs, [["Eliminar cuenta", "¿Eliminar Keep-Me y su sesión guardada? No se puede deshacer."]]);
	assert.deepEqual(notices, [["No se puede eliminar la cuenta activa. Cambia primero a otra cuenta.", "error"]]);
});
