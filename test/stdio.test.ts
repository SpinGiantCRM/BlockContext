import test from 'node:test';
import assert from 'node:assert/strict';
import {Client} from '@modelcontextprotocol/client';
import {StdioClientTransport} from '@modelcontextprotocol/client/stdio';
import {fileURLToPath} from 'node:url';
import {mkdtemp,rm,writeFile,chmod} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {at} from '../src/types.js';

async function start(env:Record<string,string>,args:string[]=[]) {
  const transport=new StdioClientTransport({command:process.execPath,args:[...args,fileURLToPath(new URL('../src/main.js',import.meta.url))],env,stderr:'pipe',cwd:tmpdir()});
  let errors='';transport.stderr!.on('data',chunk=>{errors+=String(chunk);});
  const client=new Client({name:'independent-stdio-client',version:'1.0'});
  await client.connect(transport);
  return {client,transport,errors:()=>errors};
}
test('standard stdio transport discovers all tools, returns structured data and shuts down cleanly',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'blockcontext-stdio-'));
  const app=await start({BLOCKCONTEXT_TRANSPORT:'stdio',BLOCKCONTEXT_MODE:'demo',BLOCKCONTEXT_DB:join(dir,'cache.sqlite')});const pid=app.transport.pid!;
  try {
    const tools=await app.client.listTools();assert.equal(tools.tools.length,4);assert.ok(tools.tools.every(t=>t.annotations?.readOnlyHint));
    const result=await app.client.callTool({name:'skyblock_profile_context',arguments:{player:'Z3L0R',profile:'Strawberry'}});assert.ok(!result.isError);assert.equal(at(result.structuredContent,'metadata','source'),'synthetic_demo');
    const auction=await app.client.callTool({name:'skyblock_auction_quote',arguments:{item:'SWORD'}});assert.ok(!auction.isError);assert.equal(at(auction.structuredContent,'coverage','status'),'complete');assert.equal(at(auction.structuredContent,'lowestBinCoins'),null);
    assert.ok(!app.errors().includes('Error:'));
  }finally{await app.client.close();await rm(dir,{recursive:true,force:true});}
  assert.throws(()=>process.kill(pid,0));
});
test('stdio loads a private env file by absolute path independently of client working directory',async()=>{
  const dir=await mkdtemp(join(tmpdir(),'blockcontext-env-')),path=join(dir,'private.env');
  await writeFile(path,`BLOCKCONTEXT_MODE=demo\nBLOCKCONTEXT_TRANSPORT=stdio\nBLOCKCONTEXT_DB=${join(dir,'cache.sqlite')}\n`,{mode:0o600});await chmod(path,0o600);
  const app=await start({},[`--env-file=${path}`]);
  try{assert.equal((await app.client.listTools()).tools.length,4);assert.equal(at((await app.client.callTool({name:'skyblock_profile_context',arguments:{player:'Z3L0R',profile:'Strawberry'}})).structuredContent,'metadata','source'),'synthetic_demo');}
  finally{await app.client.close();await rm(dir,{recursive:true,force:true});}
});
