import assert from 'node:assert/strict';
import { mkdir, mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import test from 'node:test';
import { registerProfileWidget } from '../src/profile-widget.ts';

const key = 'osdy-gentle:codex-accounts';

test('RPC account feed exposes only display fields and only publishes changes', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'osdy-feed-'));
  const store = join(dir, 'osdy-gentle');
  const events = new Map();
  const writes = [];
  const ui = { setWidget: (...args) => writes.push(args) };
  const rpc = { mode: 'rpc', ui };
  const quotas = new Map();
  registerProfileWidget({ on: (event, handler) => { events.set(event, handler); return () => {}; } }, () => dir, undefined, (_dir, account) => quotas.get(account));
  try {
    await events.get('session_start')(null, { mode: 'interactive', ui });
    assert.deepEqual(writes, []);
    await events.get('session_start')(null, rpc);
    assert.deepEqual(writes, [[key, ['{"v":1,"active":null,"accounts":[]}']]]);
    await mkdir(join(store, 'profiles', 'Work'), { recursive: true });
    await mkdir(join(store, 'profiles', 'Personal'));
    await writeFile(join(store, 'active-account.json'), JSON.stringify({ version: 1, profile: 'Work', accountId: 'not-display-data' }));
    await writeFile(join(store, 'bindings.json'), JSON.stringify({ version: 1, bindings: { work: { gentleProfile: 'model-profile', email: 'not-display-data' }, personal: { gentleProfile: 'fixture@example.invalid' } } }));
    await events.get('session_tree')();
    assert.deepEqual(writes[1], [key, ['{"v":1,"active":"Work","accounts":[{"name":"Personal","active":false,"binding":null,"quota":null},{"name":"Work","active":true,"binding":"model-profile","quota":null}]}']]);
    await events.get('session_compact')();
    assert.equal(writes.length, 2);
    // A hand-edited binding that isn't a Gentle profile name never reaches the app.
    assert.ok(!writes[1][1][0].includes('fixture@example.invalid'));
    quotas.set('Work', {
      primary: { windowMinutes: 300, usedPercent: 25, resetsAt: undefined },
      secondary: undefined, planType: 'not-display-data', bankedResetCount: 3, fetchedAt: 0,
    });
    await events.get('session_compact')();
    assert.deepEqual(writes[2], [key, [writes[1][1][0].replace(/\"quota\":null}]}$/, '"quota":"5h 75% left"}]}')]]);
    events.get('session_shutdown')();
    assert.deepEqual(writes.slice(3), [[key, undefined]]);
  } finally {
    events.get('session_shutdown')();
    await rm(dir, { recursive: true, force: true });
  }
});
