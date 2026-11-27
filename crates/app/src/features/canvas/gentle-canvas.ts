// Osdy Code Desktop: canvas tools. An agent opened beside a canvas tab (its
// "Agent" button) gets GENTLE_CANVAS_FILE and, through these tools, reads,
// edits and looks at that canvas while the user draws on it, after Tern's
// canvas_read / canvas_edit / canvas_shot. Each call drops a request in the
// canvas' `.agent` folder; the open canvas answers it (edits land as one undo
// step the user can take back). Without the variable nothing is registered.
// Written by the app; edits here are overwritten.
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { StringEnum } from "@earendil-works/pi-ai";
import { Type } from "typebox";
import { promises as fs } from "node:fs";
import * as path from "node:path";
import { randomUUID } from "node:crypto";
import { setTimeout as sleep } from "node:timers/promises";

const FILE = process.env.GENTLE_CANVAS_FILE;
const ANSWER_MS = 10_000;

type Json = Record<string, unknown>;

const SHAPES =
	"Shapes (page units, y grows down): " +
	"rect / ellipse {x, y, w, h, text?: label centered inside}; " +
	"line / arrow {x1, y1, x2, y2}; text {x, y (top-left), text}; note {x, y, w?=200, h?=200, text} (sticky note, yellow by default); " +
	"ink {points: [[x, y], …], closed?} (freehand strokes, handwriting); image {x, y, w, h, path}. " +
	"Style on any shape: color and fill_color (ink, red, orange, yellow, green, emerald, cyan, sky, blue, violet, pink, indigo, slate, light_slate, pale_slate, paper; they follow the light or dark theme), " +
	"fill (none, semi, solid, pattern) and fill_color (rect, ellipse and closed ink only), size 0-5 (stroke width and font size; default 1), opacity (25, 50, 75, 100), pen (pen, monoline, fountain; ink only, not highlighter).";

const OPS =
	"ops, applied in order, all or nothing: " +
	"{op: \"add\", shapes: [shape, …]} (or one shape's fields inline with type); " +
	"{op: \"update\", id | ids, …fields to change} (geometry, text, style; ink x, y move the stroke); " +
	"{op: \"move\", id | ids, dx, dy}; {op: \"delete\", id | ids}; {op: \"clear\"}; " +
	"{op: \"front\" | \"back\", id | ids}; {op: \"show\", ids | x, y, w, h} (moves the user's view there).";

// A section of every turn's system prompt: the canvas is where this agent
// answers, so "make a class diagram" means drawing it, not writing a file.
const ROLE = `You were opened from a canvas (a whiteboard) in Osdy Code Desktop and float over it; the user sees your answers there. The canvas is your output:
- Whatever the user asks you to create, draw, diagram, sketch, map, plan, list, compare, explain or solve goes ON THE CANVAS with canvas_edit, even when they don't say "on the canvas". "Make a class diagram" means drawing it with canvas_edit.
- Do not write files (Mermaid, Markdown, SVG, images, docs) for it unless the user explicitly asks for a file. Reading code or running commands to gather what you draw is fine.
- Start a turn about the canvas with canvas_read (and canvas_shot when there is ink: handwriting, sketches, math); answer questions about what they drew by reading it.
- Lay out cleanly: boxes (rect with text) about 0.55 × 18 px per character wide plus 32 px, 40 px+ apart on a grid, arrows from edge to edge, a title as text above; group related boxes by color. Keep new work inside \`view\` and clear of the user's shapes, then check it with canvas_shot and fix overlaps.
- Reply in chat briefly (a sentence or two on what you drew); the drawing is the answer.`;

// Next to each prompt, unseen by the user: where the answer goes.
const NUDGE =
	"[From Osdy Code Desktop, not the user] The user wrote this from the canvas you float over. " +
	"If it asks you to make, draw, diagram, plan, explain or solve anything, the result goes on the canvas with canvas_edit (read it first with canvas_read), not in a file or only in chat, unless they explicitly ask for a file.";

export default function gentleCanvas(pi: ExtensionAPI) {
	if (!FILE) return;
	const inbox = FILE.replace(/\.json$/i, "") + ".agent";

	pi.on("before_agent_start", (event) => {
		// A prompt section, so handlers that rebuild the prompt keep it.
		const options = event.systemPromptOptions;
		options.sections = { ...(options.sections ?? {}), canvas: ROLE };
		return { message: { customType: "gentle-canvas.context", content: NUDGE, display: false } };
	});

	/** Hands `request` to the open canvas and waits for its answer. */
	async function ask(request: Json, signal?: AbortSignal): Promise<Json> {
		await fs.mkdir(inbox, { recursive: true });
		const id = `${Date.now()}-${randomUUID().slice(0, 8)}`;
		const req = path.join(inbox, `${id}.req.json`);
		const res = path.join(inbox, `${id}.res.json`);
		// Renamed into place: the canvas never reads half a request.
		const tmp = path.join(inbox, `${id}.tmp`);
		await fs.writeFile(tmp, JSON.stringify(request));
		await fs.rename(tmp, req);
		const deadline = Date.now() + ANSWER_MS;
		while (Date.now() < deadline) {
			if (signal?.aborted) {
				await fs.rm(req, { force: true });
				throw new Error("Stopped.");
			}
			try {
				const text = await fs.readFile(res, "utf8");
				await fs.rm(res, { force: true });
				return JSON.parse(text) as Json;
			} catch {
				await sleep(80);
			}
		}
		await fs.rm(req, { force: true });
		throw new Error("The canvas didn't answer: its tab must be open in Osdy Code Desktop (a closed tab can be reopened with ⌥⌘T).");
	}

	const answer = (json: Json) => {
		if (json.ok === false) throw new Error(String(json.error ?? "the canvas refused the request"));
		return json;
	};

	pi.registerTool({
		name: "canvas_read",
		label: "Read canvas",
		description:
			"Read the canvas (whiteboard) the user has open beside you: every shape back to front with its id, type, geometry, text and style, " +
			"and `view`, the page area the user sees. Ink (handwriting, sketches) comes as a few points of each stroke: use canvas_shot to see what it says or draws.",
		promptSnippet: "Read the user's canvas: its shapes, text and the area in view",
		promptGuidelines: [
			"You are paired with a canvas (a whiteboard) open beside you in the app; the user may draw, write by hand or ask you about it at any time. Read it (canvas_read, and canvas_shot to see handwriting or the layout) before answering about it or changing it.",
			"When asked to add, solve, explain, organize or draw something on the canvas, do it there with canvas_edit: diagrams as labeled rects/ellipses joined by arrows, notes, text beside the user's work. Place new shapes inside `view`, beside what's there without covering it, and check the result with canvas_shot when the layout matters.",
			"Change or delete the user's own shapes only when asked; every edit is one step the user can undo.",
		],
		parameters: Type.Object({}),
		async execute(_id, _params, signal) {
			const json = answer(await ask({ action: "read" }, signal));
			return { content: [{ type: "text", text: JSON.stringify(json) }], details: undefined };
		},
	});

	pi.registerTool({
		name: "canvas_edit",
		label: "Edit canvas",
		description: `Add, change, move, restyle, reorder or delete shapes on the canvas the user has open beside you, as one undoable step. ${OPS} ${SHAPES} Answers with the ids added, changed and deleted.`,
		promptSnippet: "Draw on the user's canvas: add, change, move or delete shapes",
		parameters: Type.Object({
			ops: Type.Array(Type.Object({}, { additionalProperties: true }), { description: OPS }),
		}),
		// Some models send the ops as a JSON string.
		prepareArguments(args) {
			if (args && typeof args === "object" && typeof (args as Json).ops === "string") {
				try {
					return { ...(args as Json), ops: JSON.parse((args as Json).ops as string) } as never;
				} catch {
					// Left as is: validation reports it.
				}
			}
			return args as never;
		},
		executionMode: "sequential",
		async execute(_id, params, signal) {
			const json = answer(await ask({ action: "edit", ops: params.ops }, signal));
			return { content: [{ type: "text", text: JSON.stringify(json) }], details: undefined };
		},
	});

	pi.registerTool({
		name: "canvas_shot",
		label: "Look at canvas",
		description:
			"See part of the canvas as a picture (shapes on the page color, no grid or selection): what the user sees (default), all of it, or a page area {x, y, w, h}. " +
			"Use it to read handwriting and sketches and to check a layout after editing.",
		promptSnippet: "See the user's canvas as a picture (handwriting, sketches, layout)",
		parameters: Type.Object({
			area: Type.Optional(
				Type.Union([
					StringEnum(["view", "all"] as const),
					Type.Object({ x: Type.Number(), y: Type.Number(), w: Type.Number(), h: Type.Number() }),
				]),
			),
		}),
		async execute(_id, params, signal) {
			const json = answer(await ask({ action: "shot", area: params.area ?? "view" }, signal));
			const image = typeof json.image === "string" ? json.image : undefined;
			if (!image) throw new Error("the canvas sent no picture");
			const data = (await fs.readFile(image)).toString("base64");
			await fs.rm(image, { force: true });
			const area = JSON.stringify(json.area);
			return {
				content: [
					{ type: "text", text: `The canvas, page area ${area}.` },
					{ type: "image", data, mimeType: "image/png" },
				],
				details: undefined,
			};
		},
	});
}
