import test from 'node:test';
import assert from 'node:assert/strict';
import {once} from 'node:events';
import type {AddressInfo} from 'node:net';
import {Client,StreamableHTTPClientTransport} from '@modelcontextprotocol/client';
import {createHttp} from '../src/http.js';
import {ProfileService} from '../src/service.js';
import {RequestCache} from '../src/cache.js';
import {DemoUpstream,DEMO_UUID} from '../src/demo.js';
import {HypixelUpstream} from '../src/upstream.js';
import {AppError,at,object,type Upstream} from '../src/types.js';

async function connected(upstream:Upstream,now:()=>number){
  const cache=new RequestCache(':memory:',now),server=createHttp(new ProfileService(upstream,cache,now),'test');server.listen(0,'127.0.0.1');await once(server,'listening');
  const client=new Client({name:'failure-tests',version:'1'});await client.connect(new StreamableHTTPClientTransport(new URL(`http://127.0.0.1:${(server.address() as AddressInfo).port}/mcp`)));
  return {client,close:async()=>{await client.close();server.close();server.closeAllConnections();await once(server,'close');cache.close();}};
}
const query={name:'skyblock_profile_section',arguments:{player:'Z3L0R',profile:'Strawberry',section:'equipment'}};
test('real MCP failures distinguish 429 stale fallback from authentication rejection',async()=>{
  for(const code of ['RATE_LIMITED','HYPIXEL_AUTH_FAILED','HYPIXEL_KEY_EXPIRED']){
    let now=1_000_000;const up=new DemoUpstream(),raw=await up.profiles(DEMO_UUID);object(object(raw[0]!.members)[DEMO_UUID]).inventory={};up.profiles=async()=>raw;
    const app=await connected(up,()=>now);
    try{
      const first=await app.client.callTool(query);assert.ok(!first.isError);assert.equal(at(first.structuredContent,'result','data','backpackItems'),null);
      now+=301_000;up.profiles=async()=>{throw new AppError(code,'Safe failure message.',code==='RATE_LIMITED',60);};
      const result=await app.client.callTool(query);
      if(code==='RATE_LIMITED'){assert.ok(!result.isError);assert.equal(at(result.structuredContent,'metadata','stale'),true);assert.match(JSON.stringify(at(result.structuredContent,'metadata','warnings')),/RATE_LIMITED/);}
      else {assert.equal(result.isError,true);assert.match(JSON.stringify(result.content),new RegExp(code));assert.equal(result.structuredContent,undefined);}
    }finally{await app.close();}
  }
});
test('actual configured key expiry fails over MCP without making a new upstream request',async()=>{
  let now=1_000_000,calls=0;const raw=await new DemoUpstream().profiles(DEMO_UUID);
  const up=new HypixelUpstream('test-secret',async()=>{calls++;return Response.json({success:true,profiles:raw});},()=>now,{keyExpiresAt:new Date(now+300_000).toISOString()});
  up.resolve=async()=>({uuid:DEMO_UUID,name:'Z3L0R'});up.skillCatalog=async()=>({});up.itemCatalog=async()=>({});up.playerMetadata=async()=>({});
  const app=await connected(up,()=>now);
  try{
    assert.ok(!(await app.client.callTool(query)).isError);assert.equal(calls,1);now+=301_000;
    const result=await app.client.callTool(query);assert.equal(result.isError,true);assert.match(JSON.stringify(result.content),/HYPIXEL_KEY_EXPIRED/);assert.equal(calls,1);assert.ok(!JSON.stringify(result).includes('test-secret'));
  }finally{await app.close();}
});
test('ambiguous profiles and hostile inputs return actionable safe MCP errors',async()=>{
  const up=new DemoUpstream(),raw=await up.profiles(DEMO_UUID);raw.forEach(p=>{p.selected=false;p.cute_name='Same';});up.profiles=async()=>raw;
  const app=await connected(up,Date.now);
  try{
    const unspecified=await app.client.callTool({name:'skyblock_profile_context',arguments:{player:'Z3L0R'}});assert.equal(unspecified.isError,true);assert.match(JSON.stringify(unspecified.content),/PROFILE_REQUIRED/);
    const ambiguous=await app.client.callTool({name:'skyblock_profile_context',arguments:{player:'Z3L0R',profile:'Same'}});assert.equal(ambiguous.isError,true);assert.match(JSON.stringify(ambiguous.content),/PROFILE_AMBIGUOUS/);
    const hostile=await app.client.callTool({name:'skyblock_profile_context',arguments:{player:'https://evil.test'}});assert.equal(hostile.isError,true);
    const badAuction=await app.client.callTool({name:'skyblock_auction_quote',arguments:{item:'SWORD',url:'https://evil.test'}});assert.equal(badAuction.isError,true);
  }finally{await app.close();}
});
