// Osdy Code Desktop: the `browser` tool. Agents drive pages in the app's own
// browser (the floating cards the user watches) through `gentle-browser`,
// the app's command; a screenshot comes back as an image the conversation
// shows. Written by the app; edits here are overwritten.
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { StringEnum } from "@earendil-works/pi-ai";
import { Type } from "typebox";
import { execFile } from "node:child_process";
import { promises as fs } from "node:fs";

// The app's executable, filled in when the app writes this file.
const APP = __GENTLE_APP__;

const ACTIONS = ["click", "fill", "type", "press", "select", "check", "uncheck", "hover", "focus", "scroll"];
const OPS = [
	"open", "tabs", "goto", "back", "forward", "reload", "state", "eval", "snapshot",
	...ACTIONS, "wait", "screenshot", "pick", "close", "release",
];

type Json = Record<string, unknown>;

/** Runs one operation in the app; its `{ok}` or `{error}` answer. */
function run(op: Json, signal?: AbortSignal): Promise<Json> {
	const { promise, resolve, reject } = Promise.withResolvers<Json>();
	execFile(APP, ["--browser", "--json", JSON.stringify(op)], { signal, maxBuffer: 32 << 20 }, (error, stdout, stderr) => {
		const text = stdout.trim();
		if (text) {
			try {
				return resolve(JSON.parse(text) as Json);
			} catch {}
		}
		reject(new Error(stderr.trim() || error?.message || "gentle-browser gave no answer"));
	});
	return promise;
}

export default function gentleBrowser(pi: ExtensionAPI) {
	pi.registerTool({
		name: "browser",
		label: "Browser",
		description:
			"Drive a web page in the Osdy Code Desktop browser, where the user watches it. " +
			"open {url, name?, profile?, hidden?} opens a page (named so later calls find it; profile agent by default, shared for the user's sign-ins). " +
			"tabs lists every page. snapshot {tab, text?, root?} is an outline with refs (e12) to act on. " +
			"click / hover / focus / check / uncheck {tab, target}; fill {tab, target, text}; type {tab, target?, text}; select {tab, target, text: option}; " +
			"press {tab, text: keys like Enter or Meta+a}; scroll {tab, target?, dy?}; wait {tab, target, gone?, timeout?}; " +
			"goto {tab, url}; back / forward / reload / state {tab}; eval {tab, js} (an expression, or statements with return; awaited); " +
			"screenshot {tab} shows you the page; pick {tab} waits for the user to click an element; release {tab} hands it back; close {tab}. " +
			"Targets: a ref (e12), text/<text>, aria/[role/]<name>, xpath/<path>, or CSS. Take a snapshot after the page changes; refs go stale. " +
			"A link or form that loads a new page answers with that page (navigated: true); a single-page app may change a moment after the answer, so wait for what you expect before reading it.",
		parameters: Type.Object({
			op: StringEnum(OPS, { description: "What to do" }),
			tab: Type.Optional(Type.String({ description: "The page: the name it was opened with, or its number in tabs" })),
			url: Type.Optional(Type.String()),
			name: Type.Optional(Type.String({ description: "open: the page's name" })),
			profile: Type.Optional(Type.String({ description: "open: agent, shared, private, repo:<path>, session:<id>, or a profile name" })),
			hidden: Type.Optional(Type.Boolean({ description: "open: keep it out of sight (still live)" })),
			target: Type.Optional(Type.String()),
			text: Type.Optional(Type.String({ description: "fill/type text, select option, press keys" })),
			js: Type.Optional(Type.String()),
			dy: Type.Optional(Type.Number()),
			gone: Type.Optional(Type.Boolean()),
			timeout: Type.Optional(Type.Number({ description: "wait: seconds (10 by default)" })),
			root: Type.Optional(Type.String({ description: "snapshot: only under this target" })),
			nodes: Type.Optional(Type.Number({ description: "snapshot: most elements to list" })),
		}),
		async execute(_id, p, signal) {
			const op: Json = ACTIONS.includes(p.op)
				? { op: "act", action: p.op, tab: p.tab, target: p.target, text: p.text, dy: p.dy }
				: p.op === "open"
					? { op: "open", url: p.url, name: p.name, profile: p.profile, show: !p.hidden }
					: { ...p, op: p.op };
			delete op.hidden;
			const answer = await run(op, signal);
			if (answer.error) throw new Error(JSON.stringify(answer.error));
			const ok = (answer.ok ?? {}) as Json;
			if (p.op === "screenshot" && typeof ok.path === "string") {
				const data = (await fs.readFile(ok.path)).toString("base64");
				await fs.rm(ok.path, { force: true });
				return {
					content: [
						{ type: "text", text: `Screenshot of ${p.tab} (${ok.width}×${ok.height})` },
						{ type: "image", data, mimeType: "image/png" },
					],
					details: undefined,
				};
			}
			const text = typeof ok === "string" ? ok : JSON.stringify(ok, null, 1);
			return { content: [{ type: "text", text }], details: undefined };
		},
	});
}
