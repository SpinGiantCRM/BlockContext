import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { RequestCache } from '../src/cache.js';
import { AppError } from '../src/types.js';

test('concurrent requests share one upstream call; fresh cache does not poll', async () => {
  let calls = 0; const cache = new RequestCache();
  try {
    const load = async () => { calls++; await new Promise(resolve => setTimeout(resolve, 10)); return { coins: 42 }; };
    const results = await Promise.all(Array.from({ length: 20 }, () => cache.get('player', 300000, 1800000, load)));
    assert.equal(calls, 1); assert.ok(results.every(r => r.value.coins === 42));
    assert.equal((await cache.get('player', 300000, 1800000, load)).cache, 'fresh'); assert.equal(calls, 1);
  } finally { cache.close(); }
});
test('transient refresh failure gives labelled stale data and cools down repeated failures', async () => {
  let now = 0; let calls = 0; const cache = new RequestCache(':memory:', () => now);
  try {
    await cache.get('p', 300, 1800, async () => 4); now = 301;
    const fail = async () => { calls++; throw new AppError('RATE_LIMITED', 'retry', true); };
    const result = await cache.get('p', 300, 1800, fail);
    assert.equal(result.cache, 'stale'); assert.equal(result.fetchedAt, 0); assert.match(result.warning!, /RATE_LIMITED/);
    await cache.get('p', 300, 1800, fail); assert.equal(calls, 1);
    now = 1801; await assert.rejects(cache.get('p', 300, 1800, fail), (e: AppError) => e.code === 'RATE_LIMITED');
  } finally { cache.close(); }
});
test('auth failures never silently fall back to stale data', async () => {
  let now = 0; const cache = new RequestCache(':memory:', () => now);
  try {
    await cache.get('p', 10, 100, async () => 4); now = 11;
    await assert.rejects(cache.get('p', 10, 100, async () => { throw new AppError('AUTH', 'bad key'); }), (e: AppError) => e.code === 'AUTH');
  } finally { cache.close(); }
});
test('snapshot age starts at retrieval, including time spent normalizing', async () => {
  let now = 0; const cache = new RequestCache(':memory:', () => now);
  try {
    const initial = await cache.get('p', 200, 1000, async () => { now = 50; return { retrievedAt: 0 }; }, v => v.retrievedAt);
    assert.equal(initial.fetchedAt, 0);
    now = 210;
    const stale = await cache.get<{ retrievedAt: number }>('p', 200, 1000, async () => { throw new AppError('TIMEOUT', 'fail', true); }, v => v.retrievedAt);
    assert.equal(stale.cache, 'stale');
    now = 1010;
    await assert.rejects(cache.get<{ retrievedAt: number }>('p', 200, 1000, async () => { throw new AppError('TIMEOUT', 'fail', true); }, v => v.retrievedAt));
  } finally { cache.close(); }
});
test('a snapshot that expires while refreshing is not returned as stale', async () => {
  let now = 0; const cache = new RequestCache(':memory:', () => now);
  try {
    await cache.get('p', 10, 100, async () => 4); now = 11;
    await assert.rejects(cache.get('p', 10, 100, async () => { now = 101; throw new AppError('TIMEOUT', 'fail', true); }));
  } finally { cache.close(); }
});
test('disk cache survives restart, replaces values, and expires without history', async () => {
  const dir = mkdtempSync(join(tmpdir(), 'blockcontext-cache-')); const path = join(dir, 'cache.sqlite'); let now = 0;
  let cache = new RequestCache(path, () => now);
  try {
    await cache.get('p', 10, 100, async () => ({ coins: 1 })); cache.close(); cache = new RequestCache(path, () => now);
    assert.deepEqual((await cache.get('p', 10, 100, async () => ({ coins: 99 }))).value, { coins: 1 });
    now = 11; await cache.get('p', 10, 100, async () => ({ coins: 2 }));
    const db = new DatabaseSync(path); assert.equal(db.prepare('SELECT COUNT(*) AS n FROM cache').get()!.n, 1); db.close();
    assert.equal(statSync(path).mode & 0o777, 0o600);
    now = 112; assert.equal((await cache.get('p', 10, 100, async () => ({ coins: 3 }))).cache, 'miss');
  } finally { cache.close(); rmSync(dir, { recursive: true, force: true }); }
});
