/**
 * Osdy account commands, built only from dialogs Gentle Shell
 * desktop renders over RPC: select, confirm, input and notify.
 */
import {
	ACCOUNT_SWITCH_BUSY_MESSAGE,
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
import { formatFeedQuota, formatUsageLine, requestProfileCodexUsage, type ProfileUsageResult } from "./codex-usage.ts";
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
	/** Starts a quota lookup for `account`; the returned writer records its summary unless a newer lookup already did. */
	startQuota?: (account: string) => (summary: string) => void;
	resetActions?: CodexResetActions;
	authBackend?: PiAuthBackend;
	discover?: (events: EventBus, context: unknown) => Promise<ProfilesService | undefined>;
}

const NO_BINDING = "(no model profile — account only)";

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
	if (result.status === "ready") remember?.(formatFeedQuota(result.snapshot));
	return result;
}

/** Pick an account; labels carry the active marker, bound Gentle profile and optional quota. */
async function pickAccount(ctx: CommandContext, deps: Deps, title: string, withQuota: boolean): Promise<string | undefined> {
	const accounts = await listProfiles(deps.agentDir);
	if (accounts.length === 0) {
		ctx.ui.notify("No accounts yet. Use /osdy-account to save the current sign-in or import Osdy-Pi accounts.", "warning");
		return undefined;
	}
	const [active, bindings] = await Promise.all([readActiveAccount(deps.agentDir), readBindings(deps.agentDir)]);
	if (withQuota) ctx.ui.notify("Checking Codex quota for stored accounts…", "info");
	const quotas = withQuota ? await mapLimit(accounts, 4, (a) => quotaFor(deps, a)) : [];
	const labels = accounts.map((account, i) => {
		const parts = [account];
		if (active.status === "valid" && active.profile === account) parts[0] += " (active)";
		const bound = bindingFor(bindings, account);
		if (bound) parts.push(`model: ${bound}`);
		if (withQuota) parts.push(formatUsageLine(quotas[i]!));
		return parts.join(" — ");
	});
	const choice = await ctx.ui.select(title, labels);
	if (choice === undefined) return undefined;
	const index = labels.indexOf(choice);
	return index >= 0 ? accounts[index] : undefined;
}

async function ensureIdle(ctx: CommandContext): Promise<boolean> {
	if (!ctx.isIdle() && ctx.waitForIdle) {
		ctx.ui.notify("Waiting for the agent to finish before switching…", "info");
		await ctx.waitForIdle();
	}
	if (!ctx.isIdle()) {
		ctx.ui.notify(ACCOUNT_SWITCH_BUSY_MESSAGE, "warning");
		return false;
	}
	return true;
}

/**
 * Switch to an Osdy profile: account (global, every tab) then its bound Gentle
 * model profile (this session). The two steps are not transactional, so a
 * profile failure after a successful switch is reported as partial.
 */
export async function activateProfile(ctx: CommandContext, deps: Deps, account: string, { confirmFirst = true } = {}): Promise<void> {
	const bound = bindingFor(await readBindings(deps.agentDir), account);
	let service: ProfilesService | undefined;
	try {
		if (bound) {
			service = await (deps.discover ?? discoverProfilesService)(deps.events, ctx);
			if (!service) {
				const go = await ctx.ui.confirm(
					"Gentle profiles unavailable",
					`Gentle did not answer, so the model profile "${bound}" cannot be applied. Switch only the account to ${account}?`,
				);
				if (!go) return;
			}
		}
		if (confirmFirst) {
			const what = bound && service ? `account ${account} and model profile "${bound}"` : `account ${account}`;
			const ok = await ctx.ui.confirm(
				"Switch profile",
				`Switch to ${what}? The account changes for every Gentle Shell tab. Other providers stay signed in.`,
			);
			if (!ok) return;
		}
		if (!(await ensureIdle(ctx))) return;
		let result;
		try {
			result = await switchAccount(deps.agentDir, account, { guard: () => ctx.isIdle(), ...(deps.authBackend ? { authBackend: deps.authBackend } : {}) });
		} catch (error) {
			const message = error instanceof Error ? error.message : "Account switch failed.";
			ctx.ui.notify(message, message === ACCOUNT_SWITCH_BUSY_MESSAGE ? "warning" : "error");
			return;
		}
		const messages = [result.unchanged ? `Account ${result.profile} was already active.` : `Account switched to ${result.profile}.`];
		if (result.backup) messages.push(`Previous unowned sign-in saved to ${result.backup}.`);
		let level: NotifyLevel = "info";
		if (bound && service) {
			let outcome;
			try {
				outcome = await applyGentleProfile(service, bound, (t, m) => ctx.ui.confirm(t, m));
			} catch {
				ctx.ui.notify(`Account switched to ${result.profile}. Model profile "${bound}" was not applied: Gentle rejected the request.`, "warning");
				return;
			}
			switch (outcome.status) {
				case "applied":
					messages.push(`Model profile "${bound}" applied to this session.`);
					break;
				case "cancelled":
					messages.push(`Model profile "${bound}" was not applied (cancelled).`);
					level = "warning";
					break;
				case "unknown_profile":
					messages.push(`Model profile "${bound}" no longer exists; relink it in /osdy-account.`);
					level = "warning";
					break;
				case "busy":
					messages.push("Another profile apply is running; the model profile was not applied.");
					level = "warning";
					break;
				default:
					messages.push(`Model profile "${bound}" was not applied.`);
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

export async function osdyProfileCommand(args: string, ctx: CommandContext, deps: Deps): Promise<void> {
	const requested = args.trim();
	if (requested) {
		const match = (await listProfiles(deps.agentDir)).find((a) => a.toLowerCase() === requested.toLowerCase());
		if (!match) {
			ctx.ui.notify(`Unknown account: ${requested}`, "error");
			return;
		}
		return activateProfile(ctx, deps, match);
	}
	const account = await pickAccount(ctx, deps, "Switch Osdy profile (account + model)", true);
	if (account) await activateProfile(ctx, deps, account);
}

export async function osdyUsageCommand(args: string, ctx: CommandContext, deps: Deps): Promise<void> {
	const requested = args.trim();
	if (!requested) {
		ctx.ui.notify("Usage: /osdy-usage <name>", "warning");
		return;
	}
	const accounts = await listProfiles(deps.agentDir).catch(() => undefined);
	if (!accounts) {
		ctx.ui.notify("Could not list saved accounts.", "error");
		return;
	}
	const account = accounts.find((name) => name.toLowerCase() === requested.toLowerCase());
	if (!account) {
		ctx.ui.notify(`Unknown account: ${requested}`, "error");
		return;
	}
	if ((await quotaFor(deps, account)).status === "unavailable")
		ctx.ui.notify("Codex quota unavailable.", "warning");
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
	const action = await ctx.ui.select("Osdy accounts", [...ACTIONS]);
	switch (action) {
		case "Switch profile":
			return osdyProfileCommand("", ctx, deps);
		case "Link model profile to an account": {
			const account = await pickAccount(ctx, deps, "Account to link", false);
			if (!account) return;
			const profiles = await listGentleProfiles(deps.gentleConfigHome);
			if (profiles.length === 0) ctx.ui.notify("No saved Gentle model profiles found; create one in Configuration › Models › Profiles.", "warning");
			const choice = await ctx.ui.select(`Model profile for ${account}`, [NO_BINDING, ...profiles]);
			if (choice === undefined) return;
			await setBinding(deps.agentDir, account, choice === NO_BINDING ? undefined : choice);
			ctx.ui.notify(choice === NO_BINDING ? `${account} no longer changes the model profile.` : `${account} now applies model profile "${choice}".`, "info");
			return;
		}
		case "Check account quota": {
			const account = await pickAccount(ctx, deps, "Account quota", false);
			if (!account) return;
			ctx.ui.notify(`${account}: ${formatUsageLine(await quotaFor(deps, account))}`, "info");
			return;
		}
		case "Use or check banked reset": {
			if (!deps.resetActions) {
				ctx.ui.notify("Banked resets are unavailable in this Pi runtime.", "warning");
				return;
			}
			const choice = await ctx.ui.select("Banked reset for the active Codex account", [
				"Cancel", "Use a banked reset", "Check last reset attempt",
			]);
			if (choice === "Use a banked reset") await deps.resetActions.redeem();
			else if (choice === "Check last reset attempt") await deps.resetActions.check();
			return;
		}
		case "Save current sign-in as account": {
			const name = (await ctx.ui.input("Account name", "e.g. Osdy-Work"))?.trim();
			if (!name) return;
			try {
				ctx.ui.notify(`Saved current sign-in as ${await saveCurrentAs(deps.agentDir, name, deps.authBackend ? { authBackend: deps.authBackend } : {})} (now active).`, "info");
			} catch (error) {
				ctx.ui.notify(error instanceof Error ? error.message : "Could not save the account.", "error");
			}
			return;
		}
		case "Add empty account (then sign in)": {
			const name = (await ctx.ui.input("New account name", "e.g. Osdy-Personal"))?.trim();
			if (!name) return;
			try {
				const created = await createProfile(deps.agentDir, name);
				const go = await ctx.ui.confirm(
					"Switch now?",
					`Switch to ${created} now? Only Codex will be signed out. Sign in again under Configuration › Models › Providers.`,
				);
				if (go) await activateProfile(ctx, deps, created, { confirmFirst: false });
				else ctx.ui.notify(`Created ${created}.`, "info");
			} catch (error) {
				ctx.ui.notify(error instanceof Error ? error.message : "Could not create the account.", "error");
			}
			return;
		}
		case "Rename account": {
			const account = await pickAccount(ctx, deps, "Account to rename", false);
			if (!account) return;
			const name = (await ctx.ui.input(`New name for ${account}`, account))?.trim();
			if (!name) return;
			try {
				const renamed = await renameProfile(deps.agentDir, account, name);
				await moveBinding(deps.agentDir, account, renamed);
				ctx.ui.notify(`Renamed ${account} to ${renamed}.`, "info");
			} catch (error) {
				ctx.ui.notify(error instanceof Error ? error.message : "Could not rename the account.", "error");
			}
			return;
		}
		case "Remove account": {
			const account = await pickAccount(ctx, deps, "Account to remove", false);
			if (!account) return;
			if (!(await ctx.ui.confirm("Remove account", `Remove ${account} and its stored sign-in? This cannot be undone.`))) return;
			try {
				await removeProfile(deps.agentDir, account);
				await moveBinding(deps.agentDir, account, undefined);
				ctx.ui.notify(`Removed ${account}.`, "info");
			} catch (error) {
				ctx.ui.notify(error instanceof Error ? error.message : "Could not remove the account.", "error");
			}
			return;
		}
		case "Import accounts from Osdy-Pi": {
			const source = deps.osdySharedDir ?? defaultOsdySharedDir();
			const go = await ctx.ui.confirm("Import Osdy-Pi accounts", `Copy stored accounts from ${source}/osdy-pi/profiles into Gentle Shell? Existing names are kept; Osdy-Pi is not modified.`);
			if (!go) return;
			try {
				let result = await importOsdyProfiles(deps.agentDir, source);
				if (result.alreadyImported) {
					if (!(await ctx.ui.confirm("Already imported", "Osdy-Pi accounts were already imported. Import again (new names only)?"))) return;
					result = await importOsdyProfiles(deps.agentDir, source, { force: true });
				}
				const parts = [`Imported ${result.imported.length}: ${result.imported.join(", ") || "none"}.`];
				if (result.skipped.length) parts.push(`Skipped existing: ${result.skipped.join(", ")}.`);
				if (result.failed.length) parts.push(`Failed: ${result.failed.join(", ")}.`);
				ctx.ui.notify(parts.join(" "), result.failed.length ? "warning" : "info");
			} catch (error) {
				ctx.ui.notify(error instanceof Error ? error.message : "Import failed.", "error");
			}
			return;
		}
		default:
			return;
	}
}
