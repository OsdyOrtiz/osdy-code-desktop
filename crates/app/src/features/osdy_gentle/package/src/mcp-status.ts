import type { WidgetHost } from "./profile-widget.ts";

const WIDGET_KEY = "osdy-gentle:mcp-status";
const STATE = {
	CONNECTING: "connecting",
	READY: "ready",
	ERROR: "error",
	DISABLED: "disabled",
} as const;
const ERROR = {
	AUTH: "auth",
	TIMEOUT: "timeout",
	TRANSPORT: "transport",
	CONFIG: "config",
	UNKNOWN: "unknown",
} as const;

type State = (typeof STATE)[keyof typeof STATE];
type ErrorKind = (typeof ERROR)[keyof typeof ERROR];

export interface McpStatusHost extends WidgetHost {
	getMcpStatus?(): unknown;
	onMcpStatusChanged?(callback: () => void): { dispose(): void };
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === "object" && value !== null && !Array.isArray(value);
}

function isState(value: unknown): value is State {
	return Object.values(STATE).some((state) => state === value);
}

function isErrorKind(value: unknown): value is ErrorKind {
	return Object.values(ERROR).some((kind) => kind === value);
}

function encode(snapshot?: unknown): string {
	if (!Array.isArray(snapshot)) return JSON.stringify({ v: 1, available: false });
	const servers = snapshot.flatMap((entry: unknown) => {
		if (!isRecord(entry) || typeof entry.name !== "string" || !isState(entry.state)) return [];
		const { name, state, tools, error } = entry;
		return [{
			name,
			state,
			tools: typeof tools === "number" && Number.isSafeInteger(tools) && tools >= 0 ? tools : undefined,
			error: isRecord(error) && isErrorKind(error.kind) ? error.kind : undefined,
		}];
	});
	return JSON.stringify({ v: 1, available: true, servers });
}

export function registerMcpStatusWidget(host: McpStatusHost): void {
	let stopCurrent: (() => void) | undefined;
	const stop = () => {
		const previous = stopCurrent;
		stopCurrent = undefined;
		previous?.();
	};

	host.on?.("session_start", (_event, ctx) => {
		stop();
		if (ctx.mode !== "rpc") return;

		let active = true;
		let last: string | undefined;
		let subscription: { dispose(): void } | undefined;
		stopCurrent = () => {
			active = false;
			subscription?.dispose();
			ctx.ui.setWidget(WIDGET_KEY, undefined);
		};
		const publish = (snapshot?: unknown) => {
			const line = encode(snapshot);
			if (line === last) return;
			ctx.ui.setWidget(WIDGET_KEY, [line]);
			last = line;
		};

		const { getMcpStatus, onMcpStatusChanged } = host;
		if (typeof getMcpStatus !== "function" || typeof onMcpStatusChanged !== "function") {
			publish();
			return;
		}
		const refresh = () => {
			if (active) publish(getMcpStatus.call(host));
		};
		subscription = onMcpStatusChanged.call(host, refresh);
		refresh();
	});
	host.on?.("session_shutdown", stop);
}
