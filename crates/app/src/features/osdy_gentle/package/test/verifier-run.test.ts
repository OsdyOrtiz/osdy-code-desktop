import assert from "node:assert/strict";
import { mkdir, mkdtemp } from "node:fs/promises";
import { test } from "node:test";
import { registerHooks } from "node:module";
import { join } from "node:path";
import { tmpdir } from "node:os";

const hook = registerHooks({
	resolve(specifier, ctx, next) {
		const real = next(specifier, ctx);
		if (specifier !== "./account-store.ts" || !ctx.parentURL?.endsWith("/src/commands.ts")) return real;
		return { shortCircuit: true, url: "data:text/javascript," + encodeURIComponent(
			"export * from " + JSON.stringify(real.url) + ";" + (
				'export async function readStoredCodexCredential(_dir, name) {' +
				'const p = x => Buffer.from(JSON.stringify(x)).toString("base64url");' +
				'return { profile: name, expires: Date.now()+60000, access: [p({alg:"none"}),' +
				'p({"https://api.openai.com/auth":{chatgpt_account_id:"fixture-"+name}}), "synthetic"].join(".") }; }'
			)
		) };
	},
});
const { default: extension } = await import("../src/index.ts");
hook.deregister();

const usage = (used = 36) => Response.json({ plan_type: "service-metadata@example.invalid",
	rate_limit: { primary_window: { used_percent: used, limit_window_seconds: 18000 } },
});
async function setup(t, names = ["Work"], answer = usage) {
	const directory = await mkdtemp(join(tmpdir(),"agent-"));
	process.env.PI_CODING_AGENT_DIR = directory;
	await mkdir(join(directory,"osdy-gentle","profiles"),{ recursive:true });
	for (const name of names) await mkdir(join(directory,"osdy-gentle","profiles",name));
	const requests = [], hooks = new Map(), commands = new Map(), events = [];
	t.mock.method(globalThis,"fetch", async (url, init) => {
		assert.equal(url,"https://chatgpt.com/backend-api/wham/usage");
		requests.push(new Headers(init.headers).get("chatgpt-account-id"));
		return answer(requests.length);
	});
	const no = () => assert.fail("usage must not mutate sign-in or prompt");
	extension({
		events:{ emit:no },
		on(key,handle) { hooks.set(key,[...(hooks.get(key)??[]),handle]); return ()=>{}; },
		registerCommand(key,cmd) { commands.set(key,cmd); },
	});
	const ctx = { mode:"rpc", isIdle:no, ui:{ input:no, confirm:no, select:no,
		setWidget(key, lines) { if (key === "osdy-gentle:mcp-status") return; assert.equal(key,"osdy-gentle:codex-accounts"); events.push(lines ? JSON.parse(lines[0]) : null); },
		notify(...notice) { events.push(notice); },
	} };
	const session = async name => { for (const handle of hooks.get(name) ?? []) await handle({},ctx); };
	t.after(() => session("session_shutdown"));
	await session("session_start");
	return { requests, events, session, run: args => commands.get("osdy-usage").handler(args,ctx) };
}

test("tagged bulk example reports failures before the exact completion marker",async t=>{
	const h = await setup(t,["A","B"],()=>new Response(null,{status:503}));
	await h.run("--all --run abc123");
	assert.deepEqual(h.requests.toSorted(),["fixture-A","fixture-B"]);
	assert.deepEqual(h.events.slice(1),[
		["Accounts with unavailable Codex quota: 2.","warning"],
		{...h.events[0],usageRun:"abc123"},
	]);
});

test("boundary ids and named/untagged lookups cannot replace last marker",async t=>{
	const h = await setup(t);
	for (const args of ["Work --run id","--all --run x%20x","--all --run "+ "0".repeat(33),"--all --run"]) {
		await h.run(args);
		assert.deepEqual(h.events.at(-1),["Usage: /osdy-usage <name> | --all","warning"]);
	}
	assert.deepEqual(h.requests,[]);
	assert.equal(h.events.filter(x=>x?.usageRun).length,0);
	const id = "A" + "-".repeat(31);
	await h.run("--all --run "+id);
	await h.run("Work");
	await h.run("--all");
	assert.deepEqual([...new Set(h.events.filter(x=>x?.usageRun).map(x=>x.usageRun))],[id]);
	assert.equal(h.requests.length,3);
});

test("a tagged lookup retired before settlement cannot publish across session restart", {timeout:5000}, async t=>{
	const entered=Promise.withResolvers<void>();
	const release=Promise.withResolvers<void>();
	const h=await setup(t,["Work"],async n=>{
		if (n===1) { entered.resolve(); await release.promise; }
		return usage(n*10);
	});
	const old=h.run("--all --run retired");
	try {
		await Promise.race([entered.promise,old.then(()=>assert.fail("retired run did not fetch"))]);
		await h.session("session_shutdown");
		await h.session("session_start");
		await h.run("--all --run current");
		assert.equal(h.events.at(-1).usageRun,"current");
	} finally { release.resolve(); await old; }
	assert.deepEqual(h.events.filter(x=>x?.usageRun).map(x=>x.usageRun),["current"]);
});

test("overlapping distinct run ids each publish a completion", {timeout:5000}, async t=>{
	const both=Promise.withResolvers<void>();
	const release=Promise.withResolvers<void>();
	const h=await setup(t,["Work"],async n=>{
		if (n===2) both.resolve();
		await release.promise;
		return usage(n*10);
	});
	const a=h.run("--all --run from-ui");
	const b=h.run("--all --run from-cli");
	try { await Promise.race([both.promise,Promise.all([a,b]).then(()=>assert.fail("both runs did not fetch"))]); }
	finally { release.resolve(); await Promise.all([a,b]); }
	assert.deepEqual(h.events.filter(x=>x?.usageRun).map(x=>x.usageRun).toSorted(),["from-cli","from-ui"]);
});
