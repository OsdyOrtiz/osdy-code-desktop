// Osdy Code Desktop: what an agent's `gentle-browser` does inside a page. The app
// runs this before each operation (it does nothing when already loaded),
// starts one with `__gentleBrowser.run(id, name, args)`, and polls
// `__gentleBrowser.take(id)` for the answer. Refs (e12) name the elements a
// snapshot listed and stay valid until the page navigates.
(() => {
	if (window.__gentleBrowser) return;

	const refs = new Map();
	const ids = new WeakMap();
	let next = 1;
	const results = {};

	const fail = (kind, message) => Object.assign(new Error(message), { kind });

	function refOf(el) {
		let ref = ids.get(el);
		if (!ref) {
			ref = "e" + next++;
			ids.set(el, ref);
			refs.set(ref, new WeakRef(el));
		}
		return ref;
	}

	function visible(el) {
		if (!(el instanceof Element) || !el.isConnected) return false;
		const style = getComputedStyle(el);
		if (style.visibility === "hidden" || style.display === "none" || Number(style.opacity) === 0) return false;
		const box = el.getBoundingClientRect();
		return box.width > 0 && box.height > 0;
	}

	const INPUT_ROLES = {
		button: "button", submit: "button", reset: "button", image: "button",
		checkbox: "checkbox", radio: "radio", range: "slider", search: "searchbox",
	};
	function role(el) {
		const explicit = el.getAttribute("role");
		if (explicit) return explicit.split(" ")[0];
		const tag = el.tagName.toLowerCase();
		switch (tag) {
			case "a": return el.hasAttribute("href") ? "link" : null;
			case "button": case "summary": return "button";
			case "input": return INPUT_ROLES[(el.type || "text").toLowerCase()] || "textbox";
			case "textarea": return "textbox";
			case "select": return el.multiple ? "listbox" : "combobox";
			case "option": return "option";
			case "img": return "img";
			case "h1": case "h2": case "h3": case "h4": case "h5": case "h6": return "heading";
			case "nav": return "navigation";
			case "main": return "main";
			case "dialog": return "dialog";
			case "form": return "form";
			case "table": return "table";
			case "li": return "listitem";
		}
		if (el.isContentEditable && el.getAttribute("contenteditable") !== null) return "textbox";
		return null;
	}

	const clip = (text, max) => {
		const t = (text || "").replace(/\s+/g, " ").trim();
		return t.length > max ? t.slice(0, max - 1) + "…" : t;
	};

	function name(el) {
		const label = el.getAttribute("aria-label");
		if (label) return clip(label, 80);
		const by = el.getAttribute("aria-labelledby");
		if (by) {
			const text = by.split(/\s+/).map((id) => document.getElementById(id)?.innerText || "").join(" ");
			if (text.trim()) return clip(text, 80);
		}
		if (el.id && (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT")) {
			const lab = document.querySelector(`label[for="${CSS.escape(el.id)}"]`);
			if (lab) return clip(lab.innerText, 80);
		}
		const wrap = el.closest("label");
		if (wrap && wrap !== el && (el.tagName === "INPUT" || el.tagName === "SELECT")) return clip(wrap.innerText, 80);
		for (const attr of ["alt", "title", "placeholder"]) {
			const value = el.getAttribute(attr);
			if (value) return clip(value, 80);
		}
		if (el.tagName === "INPUT" && /^(button|submit|reset)$/i.test(el.type)) return clip(el.value, 80);
		return clip(el.innerText || el.textContent, 80);
	}

	const INTERACTIVE = new Set([
		"button", "link", "textbox", "checkbox", "radio", "combobox", "listbox", "option", "menuitem",
		"menuitemcheckbox", "menuitemradio", "tab", "switch", "slider", "searchbox", "spinbutton", "treeitem",
	]);
	const LANDMARK = new Set(["heading", "img", "dialog", "navigation", "main", "form", "table"]);

	function interactive(el, r) {
		if (r && INTERACTIVE.has(r)) return true;
		if (el.hasAttribute("onclick")) return true;
		const tab = el.getAttribute("tabindex");
		return tab !== null && Number(tab) >= 0;
	}

	function state(el) {
		const parts = [];
		if (el.disabled || el.getAttribute("aria-disabled") === "true") parts.push("disabled");
		if (el.type === "checkbox" || el.type === "radio") parts.push(el.checked ? "checked" : "unchecked");
		const expanded = el.getAttribute("aria-expanded");
		if (expanded !== null) parts.push(expanded === "true" ? "expanded" : "collapsed");
		const selected = el.getAttribute("aria-selected");
		if (selected === "true") parts.push("selected");
		if ((el.tagName === "INPUT" || el.tagName === "TEXTAREA") && el.type !== "checkbox" && el.type !== "radio") {
			const value = el.type === "password" ? (el.value ? "••••" : "") : clip(el.value, 60);
			if (value) parts.push(`value="${value}"`);
		}
		if (el.tagName === "SELECT") {
			const option = el.selectedOptions?.[0];
			if (option) parts.push(`value="${clip(option.label, 60)}"`);
		}
		if (el.tagName === "A" && el.href) parts.push(`href=${clip(el.getAttribute("href"), 80)}`);
		return parts;
	}

	function snapshot(opts) {
		const root = opts.root ? find(opts.root) : document.body;
		const meta = { url: location.href, title: document.title };
		if (opts.text) {
			const text = (root.innerText || "").replace(/\n{3,}/g, "\n\n").trim();
			return { ...meta, text: text.length > 20000 ? text.slice(0, 20000) + "\n…" : text };
		}
		const limit = opts.nodes || 400;
		const lines = [];
		let omitted = 0;
		const frames = [];
		const walk = (el, depth) => {
			if (!(el instanceof Element)) return;
			if (el.tagName === "SCRIPT" || el.tagName === "STYLE" || el.tagName === "NOSCRIPT") return;
			if (el.tagName === "IFRAME") {
				frames.push(el.src || "(inline)");
				return;
			}
			const shown = visible(el);
			if (!shown && el.children.length === 0) return;
			const r = role(el);
			let listed = false;
			if (shown && (interactive(el, r) || LANDMARK.has(r))) {
				if (lines.length < limit) {
					const label = name(el);
					const level = r === "heading" ? ` level=${el.tagName.slice(1)}` : "";
					const extra = state(el);
					lines.push(
						"  ".repeat(depth) +
							`- ${r || el.tagName.toLowerCase()}${label ? ` "${label}"` : ""} [ref=${refOf(el)}]${level}` +
							(extra.length ? " " + extra.join(" ") : ""),
					);
				} else {
					omitted++;
				}
				listed = true;
			} else if (shown && el.children.length === 0) {
				const text = clip(el.innerText, 120);
				if (text) {
					if (lines.length < limit) lines.push("  ".repeat(depth) + `- text "${text}"`);
					else omitted++;
				}
			}
			// What a listed control holds is its name already.
			if (listed && r && INTERACTIVE.has(r) && r !== "listbox" && r !== "dialog") return;
			for (const child of el.children) walk(child, listed ? depth + 1 : depth);
			if (el.shadowRoot) for (const child of el.shadowRoot.children) walk(child, depth);
		};
		walk(root, 0);
		return { ...meta, tree: lines.join("\n"), omitted, frames };
	}

	function smallest(candidates) {
		// The innermost match: one that contains no other match.
		return candidates.filter((el) => !candidates.some((other) => other !== el && el.contains(other)));
	}

	function find(target, quiet) {
		let found = null;
		if (target.ref) {
			const el = refs.get(target.ref)?.deref();
			if (!el || !el.isConnected) {
				if (quiet) return null;
				throw fail("not_found", `${target.ref} is gone (the page changed): take a new snapshot`);
			}
			return el;
		}
		if (target.css) {
			try {
				found = document.querySelector(target.css);
			} catch (e) {
				throw fail("bad_target", `not a CSS selector: ${target.css}`);
			}
		} else if (target.xpath) {
			found = document.evaluate(target.xpath, document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null)
				.singleNodeValue;
		} else if (target.text !== undefined) {
			const want = target.text.trim().toLowerCase();
			const all = [...document.querySelectorAll("body *")].filter(
				(el) => visible(el) && (el.innerText || "").toLowerCase().includes(want),
			);
			const best = smallest(all);
			// An exact match wins over a longer text containing it.
			found = best.find((el) => el.innerText.trim().toLowerCase() === want) || best[0] || null;
		} else if (target.aria !== undefined) {
			const want = target.aria.trim().toLowerCase();
			const all = [...document.querySelectorAll("body *")].filter((el) => {
				if (!visible(el)) return false;
				const r = role(el);
				if (!r || (target.role && r !== target.role)) return false;
				return name(el).toLowerCase().includes(want);
			});
			found = smallest(all)[0] || null;
		}
		if (!found && !quiet) throw fail("not_found", `no element matches ${JSON.stringify(target)}`);
		return found;
	}

	const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

	function center(el) {
		const box = el.getBoundingClientRect();
		return { clientX: box.left + box.width / 2, clientY: box.top + box.height / 2 };
	}

	function mouse(el, type, extra) {
		const init = { bubbles: true, cancelable: true, composed: true, view: window, button: 0, ...center(el), ...extra };
		const Ctor = type.startsWith("pointer") ? PointerEvent : MouseEvent;
		return el.dispatchEvent(new Ctor(type, { pointerId: 1, pointerType: "mouse", isPrimary: true, ...init }));
	}

	function setValue(el, value) {
		const proto = el.tagName === "TEXTAREA" ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
		const setter = Object.getOwnPropertyDescriptor(proto, "value")?.set;
		// Through the prototype's setter, so frameworks that track the value
		// (React) see the change.
		if (setter) setter.call(el, value);
		else el.value = value;
	}

	const KEYS = {
		enter: "Enter", tab: "Tab", escape: "Escape", esc: "Escape", backspace: "Backspace", delete: "Delete",
		space: " ", arrowup: "ArrowUp", arrowdown: "ArrowDown", arrowleft: "ArrowLeft", arrowright: "ArrowRight",
		up: "ArrowUp", down: "ArrowDown", left: "ArrowLeft", right: "ArrowRight", home: "Home", end: "End",
		pageup: "PageUp", pagedown: "PageDown",
	};

	function press(chord) {
		const parts = chord.split("+");
		const key = KEYS[parts[parts.length - 1].toLowerCase()] || parts[parts.length - 1];
		const mods = new Set(parts.slice(0, -1).map((m) => m.toLowerCase()));
		const init = {
			key, bubbles: true, cancelable: true, composed: true,
			metaKey: mods.has("meta") || mods.has("cmd"), ctrlKey: mods.has("control") || mods.has("ctrl"),
			altKey: mods.has("alt") || mods.has("option"), shiftKey: mods.has("shift"),
			code: key.length === 1 ? "Key" + key.toUpperCase() : key,
		};
		const el = document.activeElement || document.body;
		const go = el.dispatchEvent(new KeyboardEvent("keydown", init));
		if (go) {
			const editing = el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable;
			if ((init.metaKey || init.ctrlKey) && key.toLowerCase() === "a") {
				if (editing && el.select) el.select();
				else document.execCommand("selectAll");
			} else if (key === "Enter" && el.tagName === "INPUT" && el.form) {
				el.form.requestSubmit ? el.form.requestSubmit() : el.form.submit();
			} else if (key === "Enter" && el.tagName === "TEXTAREA") {
				document.execCommand("insertText", false, "\n");
			} else if (key === "Backspace" && editing) {
				document.execCommand("delete");
			} else if (key === "Tab") {
				const focusable = [...document.querySelectorAll("a[href],button,input,select,textarea,[tabindex]")].filter(
					(f) => visible(f) && !f.disabled && f.tabIndex >= 0,
				);
				const i = focusable.indexOf(el);
				const to = focusable[(i + (init.shiftKey ? -1 : 1) + focusable.length) % focusable.length];
				to?.focus();
			} else if (key.length === 1 && !init.metaKey && !init.ctrlKey && editing) {
				document.execCommand("insertText", false, key);
			}
		}
		el.dispatchEvent(new KeyboardEvent("keyup", init));
	}

	function typeText(el, text) {
		if (el) el.focus();
		for (const ch of text) {
			const target = document.activeElement || document.body;
			const init = { key: ch, bubbles: true, cancelable: true, composed: true };
			if (target.dispatchEvent(new KeyboardEvent("keydown", init))) {
				if (!document.execCommand("insertText", false, ch) && "value" in target) {
					setValue(target, target.value + ch);
					target.dispatchEvent(new InputEvent("input", { bubbles: true, data: ch, inputType: "insertText" }));
				}
			}
			target.dispatchEvent(new KeyboardEvent("keyup", init));
		}
	}

	async function act(action, target, text, dy) {
		const el = target ? find(target) : null;
		// A link, a form or a script may start a navigation: then the answer
		// is the next page (the app sees this one go), not this one. A form
		// posts a moment later, so its submit counts; a single-page app's
		// route change ends in `navigatesuccess` without leaving.
		let leaving = false;
		const leave = () => (leaving = true);
		const stay = () => (leaving = false);
		const navigating = (e) => {
			if (!e.hashChange) leaving = true;
		};
		const submitting = (e) =>
			setTimeout(() => {
				if (!e.defaultPrevented) leaving = true;
			}, 0);
		const nav = window.navigation;
		addEventListener("beforeunload", leave);
		document.addEventListener("submit", submitting, true);
		nav?.addEventListener("navigate", navigating);
		nav?.addEventListener("navigatesuccess", stay);
		nav?.addEventListener("navigateerror", stay);
		try {
			return await perform(el, action, text, dy, () => leaving);
		} finally {
			removeEventListener("beforeunload", leave);
			document.removeEventListener("submit", submitting, true);
			nav?.removeEventListener("navigate", navigating);
			nav?.removeEventListener("navigatesuccess", stay);
			nav?.removeEventListener("navigateerror", stay);
		}
	}

	async function perform(el, action, text, dy, leaving) {
		if (el && action !== "scroll") el.scrollIntoView({ block: "center", inline: "center" });
		switch (action) {
			case "click":
				mouse(el, "pointerover");
				mouse(el, "pointerdown");
				mouse(el, "mousedown");
				if (el.focus) el.focus({ preventScroll: true });
				mouse(el, "pointerup");
				mouse(el, "mouseup");
				el.click();
				break;
			case "hover":
				for (const type of ["pointerover", "pointerenter", "mouseover", "mouseenter", "pointermove", "mousemove"])
					mouse(el, type);
				break;
			case "focus":
				el.focus();
				break;
			case "fill":
				el.focus();
				if (el.isContentEditable) {
					document.execCommand("selectAll");
					document.execCommand("insertText", false, text ?? "");
				} else {
					setValue(el, text ?? "");
					el.dispatchEvent(new InputEvent("input", { bubbles: true, inputType: "insertReplacementText" }));
					el.dispatchEvent(new Event("change", { bubbles: true }));
				}
				break;
			case "type":
				typeText(el, text ?? "");
				break;
			case "press":
				if (el) el.focus();
				for (const chord of (text ?? "").split(/\s+/).filter(Boolean)) press(chord);
				break;
			case "select": {
				const want = (text ?? "").trim().toLowerCase();
				const option = [...el.options].find(
					(o) => o.value.toLowerCase() === want || o.label.trim().toLowerCase() === want,
				);
				if (!option) throw fail("not_found", `no option "${text}"`);
				el.value = option.value;
				el.dispatchEvent(new Event("input", { bubbles: true }));
				el.dispatchEvent(new Event("change", { bubbles: true }));
				break;
			}
			case "check":
			case "uncheck": {
				const want = action === "check";
				const checked = el.checked ?? el.getAttribute("aria-checked") === "true";
				if (checked !== want) el.click();
				break;
			}
			case "scroll":
				(el || document.scrollingElement || document.body).scrollBy({ top: dy ?? 600 });
				break;
			default:
				throw fail("bad_op", `unknown action ${action}`);
		}
		// Let the page react before the answer (a menu opening, a request, a
		// form posting); while it is leaving, wait for it to go (a navigation
		// that never happens, like a download, answers after 10 s).
		await sleep(150);
		const until = Date.now() + 10000;
		while (leaving() && Date.now() < until) await sleep(50);
		return { done: true, url: location.href, title: document.title };
	}

	async function wait(target, gone, timeoutMs) {
		const until = Date.now() + timeoutMs;
		for (;;) {
			const el = find(target, true);
			const there = !!el && visible(el);
			if (there !== gone) return { done: true, ref: el && there ? refOf(el) : undefined, url: location.href };
			if (Date.now() > until) throw fail("timeout", `${gone ? "still there" : "not there"} after ${timeoutMs / 1000} s`);
			await sleep(100);
		}
	}

	function pick() {
		return new Promise((resolve) => {
			const box = document.createElement("div");
			box.style.cssText =
				"position:fixed;pointer-events:none;z-index:2147483647;border:2px solid #ff6fb5;" +
				"background:rgba(255,111,181,.12);border-radius:4px;transition:all 60ms";
			document.documentElement.appendChild(box);
			let current = null;
			const move = (e) => {
				current = e.target;
				const r = current.getBoundingClientRect();
				Object.assign(box.style, { left: r.left + "px", top: r.top + "px", width: r.width + "px", height: r.height + "px" });
			};
			const done = (value) => {
				removeEventListener("mousemove", move, true);
				removeEventListener("click", click, true);
				removeEventListener("keydown", key, true);
				box.remove();
				resolve({ ...value, url: location.href, title: document.title });
			};
			const click = (e) => {
				e.preventDefault();
				e.stopPropagation();
				const el = e.target;
				const css = el.id ? "#" + CSS.escape(el.id) : el.tagName.toLowerCase() + (el.classList[0] ? "." + CSS.escape(el.classList[0]) : "");
				done({
					picked: {
						ref: refOf(el), selector: css, tag: el.tagName.toLowerCase(), role: role(el),
						name: name(el) || undefined, value: "value" in el ? el.value : undefined,
						text: clip(el.innerText, 200) || undefined,
					},
				});
			};
			const key = (e) => {
				if (e.key === "Escape") {
					e.preventDefault();
					done({ cancelled: true });
				}
			};
			addEventListener("mousemove", move, true);
			addEventListener("click", click, true);
			addEventListener("keydown", key, true);
		});
	}

	// An eval's value as JSON: elements by ref, anything else as text.
	function plain(value) {
		if (value === undefined) return null;
		if (value instanceof Element)
			return { element: refOf(value), tag: value.tagName.toLowerCase(), text: clip(value.innerText, 200) };
		try {
			return JSON.parse(JSON.stringify(value));
		} catch {
			return String(value);
		}
	}

	const fns = {
		snapshot,
		act,
		wait,
		pick,
		state: () => ({ url: location.href, title: document.title, loading: document.readyState !== "complete" }),
		history: (step) => {
			history.go(step);
			return { done: true };
		},
	};

	const settle = (id, promise) =>
		promise.then(
			(value) => (results[id] = { ok: value }),
			(e) => (results[id] = { error: { kind: e.kind || "failed", message: String(e?.message ?? e) } }),
		);

	window.__gentleBrowser = {
		run(id, fn, args) {
			settle(id, Promise.resolve().then(() => fns[fn](...args)));
			return "started";
		},
		// The agent's own code arrives as a function written into the script
		// the app runs (pages' CSP forbids `eval` but not the app's scripts).
		start(id, thunk) {
			settle(id, Promise.resolve().then(thunk).then(plain));
			return "started";
		},
		take(id) {
			if (!(id in results)) return "";
			const out = JSON.stringify(results[id]);
			delete results[id];
			return out;
		},
	};
})();
