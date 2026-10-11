import test from 'node:test';
import assert from 'node:assert/strict';
import { HypixelUpstream } from '../src/upstream.js';
import { DemoUpstream, DEMO_UUID } from '../src/demo.js';
import { AppError, at, object } from '../src/types.js';

function response(value: unknown, status = 200, headers: Record<string,string> = {}) { return new Response(JSON.stringify(value), { status, headers: { 'Content-Type': 'application/json', ...headers } }); }
test('missing key blocks authenticated requests before any network access', async () => {
  let calls = 0; const upstream = new HypixelUpstream('', async () => { calls++; return response({}); });
  await assert.rejects(upstream.profiles(DEMO_UUID), (e: AppError) => e.code === 'HYPIXEL_KEY_REQUIRED'); assert.equal(calls, 0);
});
test('identity lookup never sends the Hypixel key; key stays in the Hypixel header only', async () => {
  const fixture = await new DemoUpstream().profiles(DEMO_UUID); const seen: { url: string; key: string | null }[] = [];
  const upstream = new HypixelUpstream('test-secret', async (input, init) => {
    const url = String(input); seen.push({ url, key: new Headers(init?.headers).get('API-Key') });
    assert.equal(init?.redirect, 'error');
    return url.includes('mojang') ? response({ id: DEMO_UUID, name: 'Z3L0R' }) : response({ success: true, profiles: fixture });
  });
  await upstream.resolve('Z3L0R'); await upstream.profiles(DEMO_UUID);
  assert.equal(seen[0]!.key, null); assert.equal(seen[1]!.key, 'test-secret'); assert.ok(!seen[1]!.url.includes('test-secret'));
});
test('co-op records and banking transactions are removed before normalization', async () => {
  const raw = (await new DemoUpstream().profiles(DEMO_UUID))[0]!;
  object(raw.members).another_player = { private: 'unrelated co-op data' };
  raw.banking = { balance: 10, transactions: [{ amount: 99, initiator_name: 'other' }] };
  object(object(raw.members)[DEMO_UUID]).unrelated = { sensitive: true };
  const upstream = new HypixelUpstream('test-secret', async () => response({ success: true, profiles: [raw] }));
  const result = (await upstream.profiles(DEMO_UUID))[0]!;
  assert.deepEqual(Object.keys(object(result.members)), [DEMO_UUID]);
  assert.equal(at(result, 'banking', 'transactions'), undefined);
  assert.equal(at(result, 'members', DEMO_UUID, 'unrelated'), undefined);
});
test('remaining budget reserve blocks the next call until the reported reset', async () => {
  let now = 0, calls = 0;
  const upstream = new HypixelUpstream('test-secret', async () => { calls++; return response({ success: true, profiles: [] }, 200, { 'ratelimit-remaining': '2', 'ratelimit-reset': '60' }); }, () => now);
  await upstream.profiles(DEMO_UUID);
  await assert.rejects(upstream.profiles(DEMO_UUID), (e: AppError) => e.code === 'RATE_LIMITED' && e.retryAfterSeconds === 60); assert.equal(calls, 1);
  now = 60001; await upstream.profiles(DEMO_UUID); assert.equal(calls, 2);
});
test('429 creates a shared cooldown across different player requests', async () => {
  let calls = 0; const upstream = new HypixelUpstream('test-secret', async () => { calls++; return response({}, 429, { 'retry-after': '90' }); }, () => 1000);
  await assert.rejects(upstream.profiles(DEMO_UUID), (e: AppError) => e.code === 'RATE_LIMITED');
  await assert.rejects(upstream.profiles('11111111111111111111111111111111'), (e: AppError) => e.retryAfterSeconds === 90); assert.equal(calls, 1);
});
test('local conservative budget works when upstream omits rate headers', async () => {
  let calls = 0; const upstream = new HypixelUpstream('test-secret', async () => { calls++; return response({ success: true, profiles: [] }); }, () => 1000);
  for (let i = 0; i < 30; i++) await upstream.profiles(DEMO_UUID);
  await assert.rejects(upstream.profiles(DEMO_UUID), (e: AppError) => e.code === 'RATE_LIMITED'); assert.equal(calls, 30);
});
test('the verified 300-request budget can be configured without disabling its ceiling', async () => {
  let calls = 0;
  const upstream = new HypixelUpstream('test-secret', async () => { calls++; return response({ success: true, profiles: [] }); }, () => 1000, { requestsPerFiveMinutes: 300 });
  for (let i = 0; i < 300; i++) await upstream.profiles(DEMO_UUID);
  await assert.rejects(upstream.profiles(DEMO_UUID), (e: AppError) => e.code === 'RATE_LIMITED');
  assert.equal(calls, 300);
});
test('known development key expiry prevents a network request', async () => {
  let calls = 0;
  const upstream = new HypixelUpstream('test-secret', async () => { calls++; return response({}); }, () => Date.parse('2026-10-13T12:45:46+10:30'), { keyExpiresAt: '2026-10-13T12:45:46+10:30' });
  await assert.rejects(upstream.profiles(DEMO_UUID), (e: AppError) => e.code === 'HYPIXEL_KEY_EXPIRED');
  assert.equal(calls, 0);
  assert.throws(() => new HypixelUpstream('test-secret', fetch, Date.now, { requestsPerFiveMinutes: -1 }), (e: AppError) => e.code === 'INVALID_CONFIG');
});
test('errors are classified without echoing upstream messages or credentials', async () => {
  for (const [status, expected] of [[403, 'HYPIXEL_AUTH_FAILED'], [503, 'UPSTREAM_UNAVAILABLE']] as const) {
    const upstream = new HypixelUpstream('test-secret', async () => response({ error: 'test-secret' }, status));
    await assert.rejects(upstream.profiles(DEMO_UUID), (e: AppError) => e.code === expected && !e.message.includes('test-secret'));
  }
});
test('invalid, truncated, and oversized upstream responses are rejected', async () => {
  const invalid = new HypixelUpstream('test-secret', async () => new Response('{"success":'));
  await assert.rejects(invalid.profiles(DEMO_UUID), (e: AppError) => e.code === 'UPSTREAM_INVALID');
  const huge = new HypixelUpstream('test-secret', async () => new Response('x'.repeat(13 * 1024 * 1024)));
  await assert.rejects(huge.profiles(DEMO_UUID), (e: AppError) => e.code === 'UPSTREAM_TOO_LARGE');
});
