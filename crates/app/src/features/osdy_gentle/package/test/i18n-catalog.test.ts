import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import { translator } from '../src/i18n.ts';

test('English and Spanish catalog templates use identical placeholders', async () => {
  const src = await readFile(new URL('../src/i18n.ts', import.meta.url), 'utf8');
  const entries = [...src.matchAll(/^\s*("(?:[^"\\]|\\.)*"): ("(?:[^"\\]|\\.)*"),$/gm)];
  assert.ok(entries.length > 100);
  const keys = new Set();
  const placeholders = (s: string) => [...s.matchAll(/\{([\w.]+)\}/g)].map(m => m[1]).sort();
  for (const [, jsonEn, jsonEs] of entries) {
    const en = JSON.parse(jsonEn);
    const es = JSON.parse(jsonEs);
    assert.equal(keys.has(en), false, en);
    keys.add(en);
    assert.deepEqual(placeholders(es), placeholders(en), en);
    const variables = Object.fromEntries(placeholders(en).map(k => [k, '$&{literal}']));
    const expect = (f: string) => f.replace(/\{([\w.]+)\}/g, () => '$&{literal}');
    assert.equal(translator('en')(en, variables), expect(en), en);
    assert.equal(translator('es')(en, variables), expect(es), en);
  }
  console.log(`catalog templates checked: ${keys.size}`);
});
