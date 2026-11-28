import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { resolveLocale } from "../src/i18n.ts";
import { formatFeedQuota, formatUsageLine, type CodexUsageSnapshot } from "../src/codex-usage.ts";

test("locale file wins and is read again; missing or invalid files fall back to the locale environment", async t => {
	const dir = await mkdtemp(join(tmpdir(), "osdy-locale-"));
	t.after(() => rm(dir, { recursive: true, force: true }));
	const path = join(dir, "agent-locale");
	const env = { OSDY_DESKTOP_LOCALE_FILE: path, LC_ALL: "", LANG: "es_AR.UTF-8" };
	assert.equal(resolveLocale(env), "es");
	assert.equal(resolveLocale({ ...env, LC_MESSAGES: "en_US" }), "en");
	assert.equal(resolveLocale({ ...env, LC_ALL: "es_ES", LC_MESSAGES: "en_US" }), "es");
	assert.equal(resolveLocale({ ...env, LANG: "fr" }), "en");
	assert.equal(resolveLocale({}), "en");
	await writeFile(path, "system");
	assert.equal(resolveLocale(env), "es");
	await writeFile(path, "en\n");
	assert.equal(resolveLocale(env), "en");
	await writeFile(path, "es");
	assert.equal(resolveLocale({ ...env, LANG: "en_US" }), "es");
});

test("Spanish quota keeps both windows and banked credits readable", async t => {
	const dir = await mkdtemp(join(tmpdir(), "osdy-local-quota-"));
	t.after(() => rm(dir, { recursive: true, force: true }));
	const path = join(dir, "agent-locale");
	await writeFile(path, "es");
	const previous = process.env.OSDY_DESKTOP_LOCALE_FILE;
	process.env.OSDY_DESKTOP_LOCALE_FILE = path;
	t.after(() => { if (previous === undefined) delete process.env.OSDY_DESKTOP_LOCALE_FILE; else process.env.OSDY_DESKTOP_LOCALE_FILE = previous; });
	const window = { usedPercent: 0, windowMinutes: Number.MAX_SAFE_INTEGER, resetsAt: undefined };
	const snapshot: CodexUsageSnapshot = { primary: window, secondary: window, bankedResetCount: 2, fetchedAt: 0, planType: "Pro" };
	const summary = "9007199254740991m 100% disp. · 9007199254740991m 100% disp.";
	assert.equal(formatFeedQuota(snapshot), summary);
	assert.ok(summary.length <= 64);
	assert.equal(formatUsageLine({ status: "ready", profile: "Work", snapshot }), summary + " · 2 reinicios acumulados · Pro");
});
