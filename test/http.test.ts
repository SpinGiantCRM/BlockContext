import test from 'node:test';
import assert from 'node:assert/strict';
import { once } from 'node:events';
import type { AddressInfo } from 'node:net';
import { request } from 'node:http';
import { Client, StreamableHTTPClientTransport } from '@modelcontextprotocol/client';
import { RequestCache } from '../src/cache.js';
import { DemoUpstream } from '../src/demo.js';
import { createHttp } from '../src/http.js';
import { ProfileService } from '../src/service.js';
import { at } from '../src/types.js';

async function setup() {
  const cache = new RequestCache(); const upstream = new DemoUpstream(); let profileCalls = 0;
  const load = upstream.profiles.bind(upstream); upstream.profiles = async uuid => { profileCalls++; return load(uuid); };
  const server = createHttp(new ProfileService(upstream, cache), 'demo');
  server.listen(0, '127.0.0.1'); await once(server, 'listening');
  const base = `http://127.0.0.1:${(server.address() as AddressInfo).port}`;
  return { base, cache, server, calls: () => profileCalls, close: async () => { server.close(); server.closeAllConnections(); await once(server, 'close'); cache.close(); } };
}
test('real MCP client discovers seven read-only tools and validates profile, Garden and market results', async () => {
  const app = await setup(); const client = new Client({ name: 'integration-test', version: '1.0.0' });
  try {
    await client.connect(new StreamableHTTPClientTransport(new URL(app.base + '/mcp')));
    const tools = await client.listTools();
    assert.deepEqual(tools.tools.map(t => t.name).sort(), ['skyblock_auction_quote', 'skyblock_client_context','skyblock_live_context', 'skyblock_market_quote', 'skyblock_profile_context', 'skyblock_profile_section','skyblock_reference']);
    assert.ok(tools.tools.every(t => t.annotations?.readOnlyHint && t.outputSchema));
    assert.equal(app.calls(), 0, 'initialization and discovery must not fetch player data');
    const overview = await client.callTool({ name: 'skyblock_profile_context', arguments: { player: 'DemoPlayer', profile: 'Strawberry' } });
    assert.ok(!overview.isError); assert.equal(at(overview.structuredContent, 'metadata', 'source'), 'synthetic_demo');
    const detail = await client.callTool({ name: 'skyblock_profile_section', arguments: { player: 'DemoPlayer', profile: 'Strawberry', section: 'collections' } });
    assert.ok(!detail.isError); assert.equal(at(detail.structuredContent, 'result', 'data', 'collected', 'WHEAT'), 10000);
    assert.equal(app.calls(), 1);
    const garden = await client.callTool({ name: 'skyblock_profile_section', arguments: { player: 'DemoPlayer', profile: 'Strawberry', section: 'garden' } });
    assert.ok(!garden.isError); assert.equal(at(garden.structuredContent,'result','data','levelProgress','level'),1);
    const market = await client.callTool({ name: 'skyblock_market_quote', arguments: { item: 'WHEAT', quantity: 2 } });
    assert.ok(!market.isError); assert.equal(at(market.structuredContent,'bazaar','instantBuy','totalCoins'),16);
    assert.equal(app.calls(),1);
    const wrongProfile = await client.callTool({ name: 'skyblock_profile_context', arguments: { player: 'DemoPlayer', profile: 'Banana' } });
    assert.equal(wrongProfile.isError, true); assert.match(JSON.stringify(wrongProfile.content), /PROFILE_NOT_FOUND/);
    const badSection = await client.callTool({ name: 'skyblock_profile_section', arguments: { player: 'DemoPlayer', section: 'history' } });
    assert.equal(badSection.isError, true);
  } finally { await client.close(); await app.close(); }
});
test('HTTP boundaries reject DNS rebinding, foreign origins, oversized bodies and malformed JSON', async () => {
  const app = await setup();
  try {
    assert.equal((await fetch(app.base + '/health')).status, 200);
    // Node fetch does not preserve a custom Host header; send it at the HTTP layer.
    const hostStatus = await new Promise<number | undefined>((resolve, reject) => {
      const req = request(app.base + '/health', { headers: { Host: 'attacker.example' } }, res => { res.resume(); resolve(res.statusCode); });
      req.on('error', reject); req.end();
    });
    assert.equal(hostStatus, 403);
    assert.equal((await fetch(app.base + '/health', { headers: { Origin: 'https://attacker.example' } })).status, 403);
    assert.equal((await fetch(app.base + '/mcp')).status, 405);
    assert.equal((await fetch(app.base + '/mcp', { method: 'POST', body: '{}' })).status, 415);
    assert.equal((await fetch(app.base + '/mcp', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: '{' })).status, 400);
    assert.equal((await fetch(app.base + '/mcp', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: ' '.repeat(65537) })).status, 413);
    assert.equal(app.calls(), 0);
  } finally { await app.close(); }
});
