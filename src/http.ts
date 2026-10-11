import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';
import { localhostHostValidation, localhostOriginValidation, NodeStreamableHTTPServerTransport } from '@modelcontextprotocol/node';
import { createMcp } from './mcp.js';
import { ProfileService } from './service.js';
import { Diagnostics } from './diagnostics.js';

function reply(res: ServerResponse, status: number, value: unknown) {
  res.writeHead(status, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' }); res.end(JSON.stringify(value));
}
async function body(req: IncomingMessage): Promise<unknown> {
  const parts: Buffer[] = []; let length = 0;
  for await (const chunk of req) {
    length += Buffer.byteLength(chunk);
    if (length > 64 * 1024) throw new Error('body limit');
    parts.push(Buffer.from(chunk));
  }
  return JSON.parse(Buffer.concat(parts).toString('utf8'));
}

export function createHttp(service: ProfileService, mode: string, maxActive = 8, diagnostics = new Diagnostics()) {
  if (!Number.isInteger(maxActive) || maxActive < 1 || maxActive > 8) throw new Error('Concurrent requests must be between 1 and 8.');
  const validateHost = localhostHostValidation(), validateOrigin = localhostOriginValidation();
  let active = 0; const requests: number[] = [];
  const server = createServer(async (req, res) => {
    const started=performance.now();
    // Ignore routine health probes; no paths, headers or bodies are logged.
    if (req.url!=='/health') res.once('finish',()=>diagnostics.http(res.statusCode,performance.now()-started,active));
    if (!validateHost(req, res) || !validateOrigin(req, res)) return;
    if (req.url === '/health' && req.method === 'GET') { reply(res, 200, { status: 'ok', mode, scope: 'private_loopback', unaffiliatedWithHypixel: true }); return; }
    if (req.url !== '/mcp') { reply(res, 404, { error: 'not_found' }); return; }
    if (req.method !== 'POST') { res.setHeader('Allow', 'POST'); reply(res, 405, { error: 'method_not_allowed' }); return; }
    if (!req.headers['content-type']?.toLowerCase().startsWith('application/json')) { reply(res, 415, { error: 'json_required' }); return; }
    const now = Date.now(); while (requests.length && requests[0]! <= now - 60_000) requests.shift();
    if (active >= maxActive || requests.length >= 120) { res.setHeader('Retry-After', '60'); reply(res, 429, { error: 'local_rate_limit' }); return; }
    requests.push(now); active++;
    const mcp = createMcp(service,diagnostics);
    try {
      if (Number(req.headers['content-length'] ?? 0) > 64 * 1024) { reply(res, 413, { error: 'body_too_large' }); return; }
      let parsed: unknown;
      try { parsed = await body(req); } catch { reply(res, 400, { error: 'invalid_or_oversized_json' }); return; }
      const transport = new NodeStreamableHTTPServerTransport({ sessionIdGenerator: undefined, enableJsonResponse: true });
      await mcp.connect(transport);
      await transport.handleRequest(req, res, parsed);
    } catch {
      if (!res.headersSent) reply(res, 500, { error: 'internal_error' }); else res.end();
    } finally { active--; await mcp.close(); }
  });
  server.requestTimeout = 15_000;
  server.headersTimeout = 10_000;
  return server;
}
