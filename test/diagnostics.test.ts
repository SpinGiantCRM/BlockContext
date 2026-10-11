import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtempSync,readFileSync,readdirSync,rmSync,statSync,writeFileSync,symlinkSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {Diagnostics} from '../src/diagnostics.js';
import {AppError} from '../src/types.js';
import {ProfileService} from '../src/service.js';
import {DemoUpstream} from '../src/demo.js';
import {RequestCache} from '../src/cache.js';
import {createHttp} from '../src/http.js';
import {HypixelUpstream} from '../src/upstream.js';
import {Client,StreamableHTTPClientTransport} from '@modelcontextprotocol/client';

test('real HTTP MCP calls record useful measurements without request or result identities',async()=>{
 const dir=mkdtempSync(join(tmpdir(),'bc-logs-')),path=join(dir,'usage.jsonl'),log=new Diagnostics(path),cache=new RequestCache();
 const server=createHttp(new ProfileService(new DemoUpstream(),cache),'demo',2,log);await new Promise<void>(r=>server.listen(0,'127.0.0.1',r));
 const port=(server.address() as {port:number}).port,client=new Client({name:'secret-client-name',version:'1'});
 try {
  await client.connect(new StreamableHTTPClientTransport(new URL(`http://127.0.0.1:${port}/mcp`)));
  await client.callTool({name:'skyblock_profile_section',arguments:{player:'Z3L0R',profile:'Strawberry',section:'equipment'}});
  await client.callTool({name:'skyblock_profile_section',arguments:{player:'Z3L0R',profile:'Strawberry',section:'equipment'}});
  await client.callTool({name:'skyblock_profile_context',arguments:{player:'secret-player'}});
  const text=readFileSync(path,'utf8'),rows=text.trim().split('\n').map(s=>JSON.parse(s)),tools=rows.filter(r=>r.event==='tool');
  assert.equal(tools.length,3);assert.equal(tools[0].section,'equipment');assert.equal(tools[1].cache,'fresh');assert.ok(tools[0].responseBytes>0);assert.equal(tools[2].outcome,'error');
  assert.ok(rows.some(r=>r.event==='http' && r.httpStatus===200));
  for(const secret of ['Z3L0R','Strawberry','secret-player','secret-client-name','uuid','inventory','API-Key']) assert.ok(!text.includes(secret),secret);
  if(process.platform!=='win32') assert.equal(statSync(path).mode&0o777,0o600);
 }finally{await client.close();await new Promise<void>(r=>server.close(()=>r()));cache.close();rmSync(dir,{recursive:true,force:true});}
});
test('upstream quota and HTTP errors are measured without credential, URL or exception text',async()=>{
 const dir=mkdtempSync(join(tmpdir(),'bc-upstream-log-')),path=join(dir,'usage.jsonl'),log=new Diagnostics(path);
 try {
  const upstream=new HypixelUpstream('TOP_SECRET_KEY',async()=>new Response('{"secret":"payload"}',{status:429,headers:{'retry-after':'8','ratelimit-remaining':'5'}}),Date.now,{diagnostics:log});
  await assert.rejects(upstream.profiles('a'.repeat(32)),(e:unknown)=>e instanceof AppError && e.code==='RATE_LIMITED');
  log.tool('malicious-secret-tool',2,{metadata:{cache:'secret-cache',warnings:['secret-warning','Auction pages changed during the scan; pages from another snapshot were omitted.']},player:'secret-result'},new AppError('SECRET_ERROR_CODE','SECRET_ERROR_MESSAGE'),'secret-section');
  const text=readFileSync(path,'utf8'),rows=text.trim().split('\n').map(s=>JSON.parse(s));
  assert.equal(rows[0].endpoint,'profiles');assert.equal(rows[0].httpStatus,429);assert.equal(rows[0].remainingRequests,5);assert.equal(rows[1].errorCode,'INTERNAL_ERROR');assert.equal(rows[1].tool,null);assert.deepEqual(rows[1].auctionReasons,['snapshot_changed']);
  for(const secret of ['TOP_SECRET','secret','SECRET_ERROR','http','a'.repeat(32)]) if(secret!=='http') assert.ok(!text.includes(secret),secret);
 }finally{rmSync(dir,{recursive:true,force:true});}
});
test('logs rotate by size and time, expire, and never break tools when the sink is unsafe',()=>{
 const dir=mkdtempSync(join(tmpdir(),'bc-rotation-')),path=join(dir,'usage.jsonl');let now=Date.now();
 try {
  const log=new Diagnostics(path,{maxBytes:700,archives:2,now:()=>now});
  for(let i=0;i<30;i++)log.resource();
  assert.ok(readdirSync(dir).length<=3);assert.ok(readdirSync(dir).includes('usage.jsonl.2'));for(const name of readdirSync(dir))assert.ok(statSync(join(dir,name)).size<=700);
  now+=86400_001;log.lifecycle('started','live','http');assert.equal(JSON.parse(readFileSync(path,'utf8').trim()).event,'lifecycle');
  now+=15*86400_000;log.resource();assert.equal(readdirSync(dir).length,1);
  log.resource();assert.equal(readdirSync(dir).length,1);
  const protectedFile=join(dir,'protected');writeFileSync(protectedFile,'untouched');const link=join(dir,'link');
  if(process.platform!=='win32'){symlinkSync(protectedFile,link);assert.doesNotThrow(()=>new Diagnostics(link).resource());assert.equal(readFileSync(protectedFile,'utf8'),'untouched');}
  assert.doesNotThrow(()=>new Diagnostics(join(path,'invalid')).resource());
 }finally{rmSync(dir,{recursive:true,force:true});}
});
