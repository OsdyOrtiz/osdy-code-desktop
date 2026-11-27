// Osdy Code Desktop: the `screen` tool. Agents launch desktop apps, look at
// their windows and use them, through `gentle-screen` (the app's command);
// a screenshot comes back as an image the conversation shows. Input only
// reaches apps this tool launched. Written by the app; edits here are
// overwritten.
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { StringEnum } from "@earendil-works/pi-ai";
import { Type } from "typebox";
import { execFile } from "node:child_process";
import { promises as fs } from "node:fs";

// The app's executable, filled in when the app writes this file.
const APP = __GENTLE_APP__;

const OPS = ["launch", "windows", "screenshot", "click", "type", "key", "scroll", "quit"];

type Json = Record<string, unknown>;

/** Runs one operation; its `{ok}` or `{error}` answer. */
function run(op: Json, signal?: AbortSignal): Promise<Json> {
	const { promise, resolve, reject } = Promise.withResolvers<Json>();
	execFile(APP, ["--screen", "--json", JSON.stringify(op)], { signal, maxBuffer: 8 << 20 }, (error, stdout, stderr) => {
		const text = stdout.trim();
		if (text) {
			try {
				return resolve(JSON.parse(text) as Json);
			} catch {}
		}
		reject(new Error(stderr.trim() || error?.message || "gentle-screen gave no answer"));
	});
	return promise;
}

export default function gentleScreen(pi: ExtensionAPI) {
	pi.registerTool({
		name: "screen",
		label: "Screen",
		description:
			"Launch a desktop app, look at its windows and use it, to check what you built. " +
			"launch {command: [program, ...args], app?, cwd?, wait?} starts it and answers its first window (app names the app when the command hands off, e.g. ['open', '-a', 'Calculator'] with app 'Calculator'). " +
			"windows {app?, pid?} lists windows (frontmost first). screenshot {window | app | pid} shows you a window (or the whole screen with none). " +
			"click {window, x, y, button?, count?}, type {window, text}, key {window, keys: 'enter', 'cmd+s', 'ctrl+shift+tab'}, scroll {window, dy, x?, y?}: x and y are pixels of that window's screenshot. " +
			"quit {pid} stops an app you launched. Input only reaches apps launched with this tool, never the user's own. Take a screenshot after each step to check it.",
		parameters: Type.Object({
			op: StringEnum(OPS, { description: "What to do" }),
			command: Type.Optional(Type.Array(Type.String(), { description: "launch: the program and its arguments" })),
			app: Type.Optional(Type.String({ description: "An app's name (part of it)" })),
			cwd: Type.Optional(Type.String()),
			wait: Type.Optional(Type.Number({ description: "launch: seconds to wait for a window (20)" })),
			pid: Type.Optional(Type.Number()),
			window: Type.Optional(Type.Number({ description: "A window id from windows or launch" })),
			x: Type.Optional(Type.Number()),
			y: Type.Optional(Type.Number()),
			button: Type.Optional(StringEnum(["left", "right", "middle"])),
			count: Type.Optional(Type.Number({ description: "click: 2 for a double click" })),
			text: Type.Optional(Type.String()),
			keys: Type.Optional(Type.String()),
			dy: Type.Optional(Type.Number({ description: "scroll: pixels, positive is down" })),
		}),
		async execute(_id, p, signal) {
			const answer = await run({ ...p }, signal);
			if (answer.error) throw new Error(JSON.stringify(answer.error));
			const ok = (answer.ok ?? {}) as Json;
			if (p.op === "screenshot" && typeof ok.path === "string") {
				const data = (await fs.readFile(ok.path)).toString("base64");
				await fs.rm(ok.path, { force: true });
				const what = ok.window ? `window ${ok.window}` : "the screen";
				return {
					content: [
						{ type: "text", text: `Screenshot of ${what} (${ok.width}×${ok.height}; click in these pixels)` },
						{ type: "image", data, mimeType: "image/png" },
					],
					details: undefined,
				};
			}
			return { content: [{ type: "text", text: JSON.stringify(ok, null, 1) }], details: undefined };
		},
	});
}
