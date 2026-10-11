import test from 'node:test';
import assert from 'node:assert/strict';
import { DemoUpstream, DEMO_UUID } from '../src/demo.js';
import { RequestCache } from '../src/cache.js';
import { ProfileService } from '../src/service.js';
import { normalizeProfile } from '../src/normalize.js';
import { AppError, at, object, type JsonObject } from '../src/types.js';
import { uuidValue, validatePlayer } from '../src/upstream.js';

test('demo overview is unmistakably synthetic and includes the exact profile identity', async () => {
  const cache = new RequestCache();
  try {
    const result = await new ProfileService(new DemoUpstream(), cache).query('Z3L0R', 'Strawberry');
    assert.equal(at(result, 'metadata', 'source'), 'synthetic_demo');
    assert.match(String(at(result, 'metadata', 'warnings')), /invented test values/);
    assert.equal(at(result, 'profile', 'id'), '11111111111111111111111111111111');
    assert.equal(at(result, 'summary', 'accessories', 'currentMagicalPower'), null);
    assert.equal(at(result, 'summary', 'accessories', 'highestRecordedMagicalPower'), 200);
    assert.equal(at(result, 'summary', 'minions', 'placedMinions'), null);
  } finally { cache.close(); }
});
test('case-insensitive names, UUIDs, and selected profiles work; wrong profile errors', async () => {
  const cache = new RequestCache();
  try {
    const service = new ProfileService(new DemoUpstream(), cache);
    assert.equal(at(await service.query('z3l0r', 'apple'), 'profile', 'gameMode'), 'ironman');
    assert.equal(at(await service.query(DEMO_UUID, '22222222-2222-2222-2222-222222222222'), 'profile', 'name'), 'Apple');
    assert.equal(at(await service.query('Z3L0R'), 'profile', 'name'), 'Strawberry');
    await assert.rejects(service.query('Z3L0R', 'Banana'), (e: AppError) => e.code === 'PROFILE_NOT_FOUND');
  } finally { cache.close(); }
});
test('unknown data stays unavailable; zero purse is known zero; empty inventory is known empty', async () => {
  const upstream = new DemoUpstream(); const raw = (await upstream.profiles(DEMO_UUID))[1]!;
  const profile = await normalizeProfile(raw, DEMO_UUID, {});
  assert.equal(profile.sections.skills.status, 'unavailable');
  assert.equal(profile.sections.currencies.data.purseCoins, 0);
  assert.equal(profile.sections.currencies.data.sharedProfileBankCoins, null);
  assert.equal(profile.sections.minions.data.craftedGenerators, null);
  assert.equal(profile.sections.collections.status, 'unavailable');
});
test('skill XP uses current resource tables and discovers Hunting; private skill caps remain qualified', async () => {
  const upstream = new DemoUpstream(); const raw = (await upstream.profiles(DEMO_UUID))[0]!;
  const profile = await normalizeProfile(raw, DEMO_UUID, await upstream.skillCatalog());
  assert.equal(at(profile.sections.skills.data, 'skills', 'hunting', 'xpLevel'), 1);
  assert.equal(at(profile.sections.skills.data, 'skills', 'farming', 'experience'), 0);
  assert.equal(at(profile.sections.skills.data, 'skills', 'mining', 'experienceToNextLevel'), 175);
  assert.match(profile.sections.skills.warnings.join(' '), /cap gates/);
});
test('missing skill catalog preserves XP without inventing levels', async () => {
  const upstream = new DemoUpstream(); const raw = (await upstream.profiles(DEMO_UUID))[0]!;
  const profile = await normalizeProfile(raw, DEMO_UUID, {});
  assert.equal(at(profile.sections.skills.data, 'skills', 'combat', 'experience'), 400);
  assert.equal(at(profile.sections.skills.data, 'skills', 'combat', 'xpLevel'), null);
});
test('crafted minions use the current player_data field and retain the legacy fallback', async () => {
  const upstream = new DemoUpstream(); const raw = (await upstream.profiles(DEMO_UUID))[0]!;
  const member = object(object(raw.members)[DEMO_UUID]);
  object(member.player_data).crafted_generators = ['DIAMOND_1', 'DIAMOND_2'];
  const profile = await normalizeProfile(raw, DEMO_UUID, {});
  assert.deepEqual(profile.sections.minions.data.craftedGenerators, ['DIAMOND_1', 'DIAMOND_2']);
  delete object(member.player_data).crafted_generators;
  assert.deepEqual((await normalizeProfile(raw, DEMO_UUID, {})).sections.minions.data.craftedGenerators, ['WHEAT_1', 'WHEAT_2', 'COBBLESTONE_1']);
});
test('a missing selected profile requires explicit selection; malformed input cannot form upstream URLs', async () => {
  const cache = new RequestCache(); const base = new DemoUpstream();
  const upstream = { source: base.source, resolve: base.resolve.bind(base), skillCatalog: base.skillCatalog.bind(base), itemCatalog: base.itemCatalog.bind(base), playerMetadata: base.playerMetadata.bind(base), garden: base.garden.bind(base), bazaar: base.bazaar.bind(base), auctionPage: base.auctionPage.bind(base), profiles: async (uuid: string) => (await base.profiles(uuid)).map(p => ({ ...p, selected: false })) };
  try {
    await assert.rejects(new ProfileService(upstream, cache).query('Z3L0R'), (e: AppError) => e.code === 'PROFILE_REQUIRED');
    assert.throws(() => validatePlayer('https://example.com/key'), (e: AppError) => e.code === 'INVALID_PLAYER');
    assert.equal(uuidValue('111-11111111111111111111111111111'), null);
    assert.equal(uuidValue('ab8fd9cd-c4fa-4f4e-8571-c5aa1e07e614'), DEMO_UUID);
  } finally { cache.close(); }
});
test('overview and detailed sections share the same cached upstream snapshot', async () => {
  const cache = new RequestCache(); const upstream = new DemoUpstream(); let calls = 0;
  const profiles = upstream.profiles.bind(upstream); upstream.profiles = async uuid => { calls++; return profiles(uuid); };
  try {
    const service = new ProfileService(upstream, cache);
    await service.query('Z3L0R', 'Strawberry');
    const section = await service.query('Z3L0R', 'Strawberry', 'collections');
    assert.equal(calls, 1); assert.equal(at(section, 'metadata', 'cache'), 'fresh');
    assert.equal(at(section, 'result', 'data', 'collected', 'WHEAT'), 10000);
  } finally { cache.close(); }
});
test('detailed lists and collections paginate without silently losing remaining data', async () => {
  const cache = new RequestCache();
  try {
    const service = new ProfileService(new DemoUpstream(), cache);
    const first = await service.query('Z3L0R', 'Strawberry', 'minions', { limit: 1 });
    assert.deepEqual(at(first, 'result', 'data', 'craftedGenerators'), ['WHEAT_1']);
    assert.equal(at(first, 'pagination', 'craftedGenerators', 'total'), 3);
    assert.equal(at(first, 'pagination', 'craftedGenerators', 'hasMore'), true);
    const second = await service.query('Z3L0R', 'Strawberry', 'minions', { offset: 1, limit: 2 });
    assert.deepEqual(at(second, 'result', 'data', 'craftedGenerators'), ['WHEAT_2', 'COBBLESTONE_1']);
    assert.equal(at(second, 'pagination', 'craftedGenerators', 'hasMore'), false);
    const collection = await service.query('Z3L0R', 'Strawberry', 'collections', { limit: 1 });
    assert.deepEqual(at(collection, 'result', 'data', 'collected'), { WHEAT: 10000 });
    await assert.rejects(service.query('Z3L0R', 'Strawberry', 'pets', { limit: 1000000 }), (e: AppError) => e.code === 'INVALID_PAGE');
  } finally { cache.close(); }
});
