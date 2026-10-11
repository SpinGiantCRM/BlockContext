import { Client, StreamableHTTPClientTransport } from '@modelcontextprotocol/client';

const client = new Client({ name: 'BlockContext-smoke', version: '0.4.0' });
try {
  await client.connect(new StreamableHTTPClientTransport(new URL(`http://127.0.0.1:${process.env.PORT ?? 3008}/mcp`)));
  const tools = await client.listTools();
  console.log(JSON.stringify({ discoveredTools: tools.tools.map(t => t.name) }));
  const result = await client.callTool({ name: 'skyblock_profile_context', arguments: { player: process.env.BLOCKCONTEXT_TEST_PLAYER ?? 'DemoPlayer', profile: process.env.BLOCKCONTEXT_TEST_PROFILE ?? 'Strawberry' } });
  console.log(JSON.stringify(result.structuredContent ?? result.content, null, 2));
  if (result.isError) process.exitCode = 1;
} finally { await client.close(); }
