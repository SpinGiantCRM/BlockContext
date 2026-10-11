import { RequestCache } from './cache.js';
import { DemoUpstream } from './demo.js';
import { createHttp } from './http.js';
import { ProfileService } from './service.js';
import { HypixelUpstream } from './upstream.js';
import { StdioServerTransport } from '@modelcontextprotocol/server/stdio';
import { createMcp } from './mcp.js';

const transport = process.env.BLOCKCONTEXT_TRANSPORT ?? 'http';
if (transport !== 'http' && transport !== 'stdio') throw new Error('BLOCKCONTEXT_TRANSPORT must be http or stdio.');

const mode = process.env.BLOCKCONTEXT_MODE ?? 'demo';
if (mode !== 'demo' && mode !== 'live') throw new Error('BLOCKCONTEXT_MODE must be demo or live.');
if (mode === 'live' && !process.env.HYPIXEL_API_KEY) throw new Error('Live mode requires HYPIXEL_API_KEY in the private local .env file.');
const port = Number(process.env.PORT ?? 3008);
if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error('PORT must be between 1 and 65535.');
const cache = new RequestCache(process.env.BLOCKCONTEXT_DB ?? '.data/cache.sqlite');
const upstream = mode === 'demo' ? new DemoUpstream() : new HypixelUpstream(process.env.HYPIXEL_API_KEY!, fetch, Date.now, { requestsPerFiveMinutes: process.env.HYPIXEL_REQUESTS_PER_5_MINUTES ? Number(process.env.HYPIXEL_REQUESTS_PER_5_MINUTES) : undefined, keyExpiresAt: process.env.HYPIXEL_API_KEY_EXPIRES_AT });
const service = new ProfileService(upstream,cache);
const server = transport==='http' ? createHttp(service, mode, Number(process.env.BLOCKCONTEXT_MAX_CONCURRENT ?? 8)) : null;
const mcp = transport==='stdio' ? createMcp(service) : null;
let stopping = false;
const stop = async () => {
  if (stopping) return; stopping = true;
  if (server) { server.close(() => { cache.close(); process.exit(0); }); server.closeIdleConnections(); }
  else { await mcp!.close(); cache.close(); process.exit(0); }
};
for (const signal of ['SIGINT', 'SIGTERM'] as const) process.on(signal, () => { void stop(); });
if (server) server.listen(port,'127.0.0.1',()=>{
  console.error(`BlockContext ${mode} listening locally at http://127.0.0.1:${port}/mcp`);
  if (mode==='demo') console.error('SYNTHETIC DEMO: invented sample values, not live player stats.');
});
else {
  const stdio = new StdioServerTransport();
  mcp!.server.onclose = () => { void stop(); };
  await mcp!.connect(stdio);
  process.stdin.once('end',()=>{void stop();});
}
