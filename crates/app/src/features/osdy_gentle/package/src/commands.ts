/**
 * Osdy account commands, built only from dialogs Gentle Shell
 * desktop renders over RPC: select, confirm, input and notify.
 */
import { resolveLocale, translator, type Locale, type Translate } from "./i18n.ts";
import {
	AccountSwitchBusyError,
	createProfile,
	importOsdyProfiles,
	listProfiles,
	readActiveAccount,
	readStoredCodexCredential,
	removeProfile,
	renameProfile,
	saveCurrentAs,
	switchAccount,
	defaultOsdySharedDir,
} from "./account-store.ts";
import { bindingFor, moveBinding, readBindings, setBinding } from "./bindings.ts";
import type { CodexResetActions } from "./codex-reset.ts";
import type { PiAuthBackend } from "./pi-auth-backend.ts";
import { formatUsageLine, requestProfileCodexUsage, type CodexUsageSnapshot, type ProfileUsageResult } from "./codex-usage.ts";
import {
	applyGentleProfile,
	discoverProfilesService,
	listGentleProfiles,
	type EventBus,
	type ProfilesService,
} from "./gentle-profiles.ts";

export type NotifyLevel = "info" | "warning" | "error";

export interface CommandUI {
	select(title: string, options: string[]): Promise<string | undefined>;
	confirm(title: string, message: string): Promise<boolean>;
	input(title: string, placeholder?: string): Promise<string | undefined>;
	notify(message: string, type?: NotifyLevel): void;
}

export interface CommandContext {
	ui: CommandUI;
	hasUI?: boolean;
	isIdle(): boolean;
	waitForIdle?: () => Promise<void>;
}

export interface Deps {
	agentDir: string;
	events: EventBus;
	osdySharedDir?: string;
	gentleConfigHome?: string;
	fetch?: typeof globalThis.fetch;
	/** Starts a quota lookup for `account`; the returned writer records its snapshot unless a newer lookup already did. */
	startQuota?: (account: string) => (snapshot: CodexUsageSnapshot) => void;
	resetActions?: CodexResetActions;
	authBackend?: PiAuthBackend;
	discover?: (events: EventBus, context: unknown) => Promise<ProfilesService | undefined>;
}

async function mapLimit<T, R>(items: T[], limit: number, fn: (item: T) => Promise<R>): Promise<R[]> {
	const results: R[] = new Array(items.length);
	let next = 0;
	await Promise.all(
		Array.from({ length: Math.min(limit, items.length) }, async () => {
			while (next < items.length) {
				const index = next++;
				results[index] = await fn(items[index]!);
			}
		}),
	);
	return results;
}

async function quotaFor(deps: Deps, profile: string): Promise<ProfileUsageResult> {
	const remember = deps.startQuota?.(profile);
	const result = await requestProfileCodexUsage(profile, (p) => readStoredCodexCredential(deps.agentDir, p), deps.fetch ? { fetch: deps.fetch } : {});
	if (result.status === "ready") remember?.(result.snapshot);
	return result;
}

/** Pick an account; labels carry the active marker, bound Gentle profile and optional quota. */
async function pickAccount(ctx: CommandContext, deps: Deps, title: string, withQuota: boolean, locale: Locale): Promise<string | undefined> {
	const t = translator(locale);
	const accounts = await listProfiles(deps.agentDir);
	if (accounts.length === 0) {
		ctx.ui.notify(t("No accounts yet. Use /osdy-account to save the current sign-in or import Osdy-Pi accounts."), "warning");
		return undefined;
	}
	const [active, bindings] = await Promise.all([readActiveAccount(deps.agentDir), readBindings(deps.agentDir)]);
	if (withQuota) ctx.ui.notify(t("Checking Codex quota for stored accounts…"), "info");
	const quotas = withQuota ? await mapLimit(accounts, 4, (a) => quotaFor(deps, a)) : [];
	const labels = accounts.map((account, i) => {
		const parts = [account];
		if (active.status === "valid" && active.profile === account) parts[0] += t(" (active)");
		const bound = bindingFor(bindings, account);
		if (bound) parts.push(t("model: {bound}", { bound }));
		if (withQuota) parts.push(formatUsageLine(quotas[i]!, locale));
		return parts.join(" — ");
	});
	const choice = await ctx.ui.select(title, labels);
	if (choice === undefined) return undefined;
	const index = labels.indexOf(choice);
	return index >= 0 ? accounts[index] : undefined;
}

async function ensureIdle(ctx: CommandContext, t: Translate): Promise<boolean> {
	if (!ctx.isIdle() && ctx.waitForIdle) {
		ctx.ui.notify(t("Waiting for the agent to finish before switching…"), "info");
		await ctx.waitForIdle();
	}
	if (!ctx.isIdle()) {
		ctx.ui.notify(new AccountSwitchBusyError(t).message, "warning");
		return false;
	}
	return true;
}

/**
 * Switch to an Osdy profile: account (global, every tab) then its bound Gentle
 * model profile (this session). The two steps are not transactional, so a
 * profile failure after a successful switch is reported as partial.
 */
export async function activateProfile(ctx: CommandContext, deps: Deps, account: string, { confirmFirst = true, locale = resolveLocale() } = {}): Promise<void> {
	const t = translator(locale);
	const bound = bindingFor(await readBindings(deps.agentDir), account);
	let service: ProfilesService | undefined;
	try {
		if (bound) {
			service = await (deps.discover ?? discoverProfilesService)(deps.events, ctx);
			if (!service) {
				const go = await ctx.ui.confirm(
					t("Gentle profiles unavailable"),
					t("Gentle did not answer, so the model profile \"{bound}\" cannot be applied. Switch only the account to {account}?", { bound, account }),
				);
				if (!go) return;
			}
		}
		if (confirmFirst) {
			const what = bound && service ? t("account {account} and model profile \"{bound}\"", { account, bound }) : t("account {account}", { account });
			const ok = await ctx.ui.confirm(
				t("Switch profile"),
				t("Switch to {what}? The account changes for every Gentle Shell tab. Other providers stay signed in.", { what }),
			);
			if (!ok) return;
		}
		if (!(await ensureIdle(ctx, t))) return;
		let result;
		try {
			result = await switchAccount(deps.agentDir, account, { guard: () => ctx.isIdle(), ...(deps.authBackend ? { authBackend: deps.authBackend } : {}) });
		} catch (error) {
			const message = error instanceof Error ? error.message : t("Account switch failed.");
			ctx.ui.notify(message, error instanceof AccountSwitchBusyError ? "warning" : "error");
			return;
		}
		const messages = [result.unchanged ? t("Account {result.profile} was already active.", { "result.profile": result.profile }) : t("Account switched to {result.profile}.", { "result.profile": result.profile })];
		if (result.backup) messages.push(t("Previous unowned sign-in saved to {result.backup}.", { "result.backup": result.backup }));
		let level: NotifyLevel = "info";
		if (bound && service) {
			let outcome;
			try {
				outcome = await applyGentleProfile(service, bound, (t, m) => ctx.ui.confirm(t, m));
			} catch {
				ctx.ui.notify(t("Account switched to {result.profile}. Model profile \"{bound}\" was not applied: Gentle rejected the request.", { "result.profile": result.profile, bound }), "warning");
				return;
			}
			switch (outcome.status) {
				case "applied":
					messages.push(t("Model profile \"{bound}\" applied to this session.", { bound }));
					break;
				case "cancelled":
					messages.push(t("Model profile \"{bound}\" was not applied (cancelled).", { bound }));
					level = "warning";
					break;
				case "unknown_profile":
					messages.push(t("Model profile \"{bound}\" no longer exists; relink it in /osdy-account.", { bound }));
					level = "warning";
					break;
				case "busy":
					messages.push(t("Another profile apply is running; the model profile was not applied."));
					level = "warning";
					break;
				default:
					messages.push(t("Model profile \"{bound}\" was not applied.", { bound }));
					level = "warning";
			}
			if (outcome.status === "applied" || outcome.status === "not_applied")
				for (const note of outcome.notes) messages.push(note.text);
		}
		ctx.ui.notify(messages.join(" "), level);
	} finally {
		service?.dispose();
	}
}

export async function osdyProfileCommand(args: string, ctx: CommandContext, deps: Deps, locale = resolveLocale()): Promise<void> {
	const t = translator(locale);
	const requested = args.trim();
	if (requested) {
		const match = (await listProfiles(deps.agentDir)).find((a) => a.toLowerCase() === requested.toLowerCase());
		if (!match) {
			ctx.ui.notify(t("Unknown account: {requested}", { requested }), "error");
			return;
		}
		return activateProfile(ctx, deps, match, { locale });
	}
	const account = await pickAccount(ctx, deps, t("Switch Osdy profile (account + model)"), true, locale);
	if (account) await activateProfile(ctx, deps, account, { locale });
}

export async function osdyUsageCommand(args: string, ctx: CommandContext, deps: Deps): Promise<void> {
	const t = translator();
	const requested = args.trim();
	if (!requested) {
		ctx.ui.notify(t("Usage: /osdy-usage <name>"), "warning");
		return;
	}
	const accounts = await listProfiles(deps.agentDir).catch(() => undefined);
	if (!accounts) {
		ctx.ui.notify(t("Could not list saved accounts."), "error");
		return;
	}
	const account = accounts.find((name) => name.toLowerCase() === requested.toLowerCase());
	if (!account) {
		ctx.ui.notify(t("Unknown account: {requested}", { requested }), "error");
		return;
	}
	if ((await quotaFor(deps, account)).status === "unavailable")
		ctx.ui.notify(t("Codex quota unavailable."), "warning");
}

const ACTIONS = [
	"Switch profile",
	"Link model profile to an account",
	"Check account quota",
	"Use or check banked reset",
	"Save current sign-in as account",
	"Add empty account (then sign in)",
	"Rename account",
	"Remove account",
	"Import accounts from Osdy-Pi",
] as const;

export async function osdyAccountCommand(_args: string, ctx: CommandContext, deps: Deps): Promise<void> {
	const locale = resolveLocale();
	const t = translator(locale);
	const labels = ACTIONS.map(action => t(action));
	const action = await ctx.ui.select(t("Osdy accounts"), labels);
	switch (action === undefined ? undefined : ACTIONS[labels.indexOf(action)]) {
		case "Switch profile":
			return osdyProfileCommand("", ctx, deps, locale);
		case "Link model profile to an account": {
			const account = await pickAccount(ctx, deps, t("Account to link"), false, locale);
			if (!account) return;
			const profiles = await listGentleProfiles(deps.gentleConfigHome);
			if (profiles.length === 0) ctx.ui.notify(t("No saved Gentle model profiles found; create one in Configuration › Models › Profiles."), "warning");
			const noBinding = t("(no model profile — account only)");
			const choice = await ctx.ui.select(t("Model profile for {account}", { account }), [noBinding, ...profiles]);
			if (choice === undefined) return;
			await setBinding(deps.agentDir, account, choice === noBinding ? undefined : choice);
			ctx.ui.notify(choice === noBinding ? t("{account} no longer changes the model profile.", { account }) : t("{account} now applies model profile \"{choice}\".", { account, choice }), "info");
			return;
		}
		case "Check account quota": {
			const account = await pickAccount(ctx, deps, t("Account quota"), false, locale);
			if (!account) return;
			ctx.ui.notify(`${account}: ${formatUsageLine(await quotaFor(deps, account), locale)}`, "info");
			return;
		}
		case "Use or check banked reset": {
			if (!deps.resetActions) {
				ctx.ui.notify(t("Banked resets are unavailable in this Pi runtime."), "warning");
				return;
			}
			const use = t("Use a banked reset");
			const check = t("Check last reset attempt");
			const choice = await ctx.ui.select(t("Banked reset for the active Codex account"), [t("Cancel"), use, check]);
			if (choice === use) await deps.resetActions.redeem();
			else if (choice === check) await deps.resetActions.check();
			return;
		}
		case "Save current sign-in as account": {
			const name = (await ctx.ui.input(t("Account name"), t("e.g. Osdy-Work")))?.trim();
			if (!name) return;
			try {
				const saved = await saveCurrentAs(deps.agentDir, name, deps.authBackend ? { authBackend: deps.authBackend } : {});
				ctx.ui.notify(t("Saved current sign-in as {saved} (now active).", { saved }), "info");
			} catch (error) {
				ctx.ui.notify(error instanceof Error ? error.message : t("Could not save the account."), "error");
			}
			return;
		}
		case "Add empty account (then sign in)": {
			const name = (await ctx.ui.input(t("New account name"), t("e.g. Osdy-Personal")))?.trim();
			if (!name) return;
			try {
				const created = await createProfile(deps.agentDir, name);
				const go = await ctx.ui.confirm(
					t("Switch now?"),
					t("Switch to {created} now? Only Codex will be signed out. Sign in again under Configuration › Models › Providers.", { created }),
				);
				if (go) await activateProfile(ctx, deps, created, { confirmFirst: false, locale });
				else ctx.ui.notify(t("Created {created}.", { created }), "info");
			} catch (error) {
				ctx.ui.notify(error instanceof Error ? error.message : t("Could not create the account."), "error");
			}
			return;
		}
		case "Rename account": {
			const account = await pickAccount(ctx, deps, t("Account to rename"), false, locale);
			if (!account) return;
			const name = (await ctx.ui.input(t("New name for {account}", { account }), account))?.trim();
			if (!name) return;
			try {
				const renamed = await renameProfile(deps.agentDir, account, name);
				await moveBinding(deps.agentDir, account, renamed);
				ctx.ui.notify(t("Renamed {account} to {renamed}.", { account, renamed }), "info");
			} catch (error) {
				ctx.ui.notify(error instanceof Error ? error.message : t("Could not rename the account."), "error");
			}
			return;
		}
		case "Remove account": {
			const account = await pickAccount(ctx, deps, t("Account to remove"), false, locale);
			if (!account) return;
			if (!(await ctx.ui.confirm(t("Remove account"), t("Remove {account} and its stored sign-in? This cannot be undone.", { account })))) return;
			try {
				await removeProfile(deps.agentDir, account);
				await moveBinding(deps.agentDir, account, undefined);
				ctx.ui.notify(t("Removed {account}.", { account }), "info");
			} catch (error) {
				ctx.ui.notify(error instanceof Error ? error.message : t("Could not remove the account."), "error");
			}
			return;
		}
		case "Import accounts from Osdy-Pi": {
			const source = deps.osdySharedDir ?? defaultOsdySharedDir();
			const go = await ctx.ui.confirm(t("Import Osdy-Pi accounts"), t("Copy stored accounts from {source}/osdy-pi/profiles into Gentle Shell? Existing names are kept; Osdy-Pi is not modified.", { source }));
			if (!go) return;
			try {
				let result = await importOsdyProfiles(deps.agentDir, source);
				if (result.alreadyImported) {
					if (!(await ctx.ui.confirm(t("Already imported"), t("Osdy-Pi accounts were already imported. Import again (new names only)?")))) return;
					result = await importOsdyProfiles(deps.agentDir, source, { force: true });
				}
				const parts = [t("Imported {count}: {names}.", { count: result.imported.length, names: result.imported.join(", ") || t("none") })];
				if (result.skipped.length) parts.push(t("Skipped existing: {names}.", { names: result.skipped.join(", ") }));
				if (result.failed.length) parts.push(t("Failed: {names}.", { names: result.failed.join(", ") }));
				ctx.ui.notify(parts.join(" "), result.failed.length ? "warning" : "info");
			} catch (error) {
				ctx.ui.notify(error instanceof Error ? error.message : t("Import failed."), "error");
			}
			return;
		}
		default:
			return;
	}
}
