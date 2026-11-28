/**
 * Gentle model profiles: list names from Gentle's `profiles.json` and apply one
 * for the current session through Gentle's versioned profiles-service event,
 * the same owner action as Configuration → Models → "Use in this session".
 */
import { t } from "./i18n.ts";
import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { isAbsolute, join } from "node:path";

export const PROFILES_SERVICE_SCHEMA = "gentle-pi.desktop-profiles-service/v1";
export const PROFILES_SERVICE_CHANNEL = "gentle:desktop:profiles-service";
const DISCOVERY_TIMEOUT_MS = 100;

export type ProfileNote = { level: "info" | "warning" | "error"; text: string };

export type ApplyResult =
	| { status: "confirmation_required"; title: string; message: string; confirmation: string }
	| { status: "applied" | "not_applied"; notes: ProfileNote[] }
	| { status: "unknown_profile" }
	| { status: "busy" };

export interface ProfilesService {
	schema: typeof PROFILES_SERVICE_SCHEMA;
	apply(request: { profile: string; mode: "session" | "default"; confirmation?: string }): Promise<ApplyResult>;
	dispose(): void;
}

export interface EventBus {
	emit(channel: string, data: unknown): void;
}

/** Gentle's config home, same precedence as gentle-pi's resolveGentlePiConfigHome. */
export function resolveGentleConfigHome(env: NodeJS.ProcessEnv = process.env, home: string = homedir()): string {
	const prefix = env.GENTLE_SHELL_PREFIX;
	if (prefix && existsSync(join(prefix, "config", "gentle-ai"))) return join(prefix, "config", "gentle-ai");
	if (env.GENTLE_PI_CONFIG_HOME) return env.GENTLE_PI_CONFIG_HOME;
	const marker = env.GENTLE_SHELL_CONFIG_HOME;
	if (marker && isAbsolute(marker)) return marker;
	return join(home, ".pi", "gentle-ai");
}

/** Saved Gentle profile names (empty when the store is missing or unreadable). */
export async function listGentleProfiles(configHome: string = resolveGentleConfigHome()): Promise<string[]> {
	try {
		const parsed = JSON.parse(await readFile(join(configHome, "profiles.json"), "utf8"));
		if (!parsed || parsed.kind !== "gentle-pi.agent_model_profiles" || typeof parsed.profiles !== "object" || !parsed.profiles)
			return [];
		return Object.keys(parsed.profiles).sort();
	} catch {
		return [];
	}
}

function isService(value: unknown): value is ProfilesService {
	if (!value || typeof value !== "object") return false;
	const s = value as Partial<ProfilesService>;
	return s.schema === PROFILES_SERVICE_SCHEMA && typeof s.apply === "function" && typeof s.dispose === "function";
}

/** Discover Gentle's live profiles owner for this session; undefined when Gentle does not answer. */
export function discoverProfilesService(events: EventBus, context: unknown, timeoutMs = DISCOVERY_TIMEOUT_MS): Promise<ProfilesService | undefined> {
	return new Promise((resolve) => {
		let settled = false;
		const timer = setTimeout(() => {
			settled = true;
			resolve(undefined);
		}, timeoutMs);
		const respond = (value: unknown) => {
			if (!isService(value)) return;
			if (settled) {
				value.dispose();
				return;
			}
			settled = true;
			clearTimeout(timer);
			resolve(value);
		};
		try {
			events.emit(PROFILES_SERVICE_CHANNEL, { context, respond });
		} catch {
			settled = true;
			clearTimeout(timer);
			resolve(undefined);
		}
	});
}

export type ApplyOutcome =
	| { status: "applied"; notes: ProfileNote[] }
	| { status: "not_applied"; notes: ProfileNote[] }
	| { status: "cancelled" }
	| { status: "unknown_profile" }
	| { status: "busy" }
	| { status: "unavailable" };

/**
 * Apply a Gentle profile to this session. A confirmation the owner asks for is
 * shown verbatim and resent only with the human's acceptance.
 */
export async function applyGentleProfile(
	service: ProfilesService,
	profile: string,
	confirm: (title: string, message: string) => Promise<boolean>,
): Promise<ApplyOutcome> {
	let result = await service.apply({ profile, mode: "session" });
	if (result.status === "confirmation_required") {
		if (!(await confirm(result.title, result.message))) return { status: "cancelled" };
		result = await service.apply({ profile, mode: "session", confirmation: result.confirmation });
		if (result.status === "confirmation_required") return { status: "not_applied", notes: [{ level: "warning", text: t("Gentle asked for another confirmation; nothing was applied.") }] };
	}
	return result;
}
