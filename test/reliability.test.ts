import test from 'node:test';
import assert from 'node:assert/strict';
import {AuctionService} from '../src/auctions.js';
import {RequestCache} from '../src/cache.js';
import {DemoUpstream} from '../src/demo.js';
import {HypixelUpstream} from '../src/upstream.js';
import {dungeonsSection} from '../src/progression.js';
import {AppError,at,type JsonObject} from '../src/types.js';

const NOW=1_000_000;
const page=(n:number,total=3,updated=NOW):JsonObject=>({page:n,totalPages:total,totalAuctions:0,lastUpdated:updated,auctions:[]});
async function scan(fetchPage:(n:number,signal?:AbortSignal)=>Promise<JsonObject>,budget=45_000) {
  const up=new DemoUpstream(),cache=new RequestCache(':memory:',()=>NOW);up.auctionPage=fetchPage;
  try{return await new AuctionService(up,cache,()=>NOW,budget).quote('HYPERION');}finally{cache.close();}
}
test('Catacombs coverage preserves a successful null reason and distinguishes missing XP and unsupported types',()=>{
  const result=dungeonsSection({dungeons:{dungeon_types:{catacombs:{experience:50},future_dungeon:{experience:50}}}});
  assert.equal(at(result,'data','fieldCoverage','catacombs','level','status'),'available');
  assert.equal(at(result,'data','fieldCoverage','catacombs','level','reasonCode'),null);
  assert.equal(at(result,'data','fieldCoverage','future_dungeon','level','reasonCode'),'unsupported_dungeon_type');
  const missing=dungeonsSection({dungeons:{dungeon_types:{catacombs:{}}}});
  assert.equal(at(missing,'data','fieldCoverage','catacombs','level','reasonCode'),'missing_or_invalid_experience');
  assert.equal(at(result,'data','types','catacombs','nextUnlock','referenceDate'),'2021-03-30');
  assert.equal(at(result,'data','types','catacombs','nextUnlock','referenceReview','checkedAt'),'2026-10-11');
  assert.equal(at(result,'data','floorRequirementReview','status'),'historical_reference_current_rules_unverified');
});
test('one transient page failure recovers within the same snapshot and reports retry coverage',async()=>{
  const calls=new Map<number,number>();
  const q=await scan(async n=>{const count=(calls.get(n)??0)+1;calls.set(n,count);if(n===1&&count===1)throw new AppError('UPSTREAM_TIMEOUT','timeout',true);return page(n);});
  assert.equal(at(q,'coverage','status'),'complete');assert.equal(at(q,'coverage','retryAttempts'),1);assert.equal(at(q,'coverage','recoveredPageFetches'),1);assert.equal(at(q,'coverage','failedPageFetches'),0);assert.equal(calls.get(1),2);
});
test('a recovered request from a changed snapshot stays partial and cannot provide a global lowest BIN',async()=>{
  let calls=0;const q=await scan(async n=>{if(n===1&&++calls===1)throw new AppError('UPSTREAM_UNAVAILABLE','offline',true);return page(n,3,n===1?NOW+1:NOW);});
  assert.equal(at(q,'coverage','status'),'partial');assert.equal(at(q,'coverage','scannedPages'),2);assert.equal(q.lowestBinCoins,null);assert.equal(at(q,'coverage','recoveredPageFetches'),1);
});
test('two persistent page failures have bounded retries and one identical warning',async()=>{
  const calls=new Map<number,number>();const q=await scan(async n=>{calls.set(n,(calls.get(n)??0)+1);if(n)throw new AppError('UPSTREAM_UNAVAILABLE','offline',true);return page(n);});
  assert.equal(at(q,'coverage','retryAttempts'),2);assert.equal(at(q,'coverage','failedPageFetches'),2);assert.equal(at(q,'coverage','recoveredPageFetches'),0);assert.equal(calls.get(1),2);assert.equal(calls.get(2),2);
  const warnings=at(q,'metadata','warnings') as string[];assert.equal(warnings.filter(v=>v==='One or more auction pages could not be retrieved.').length,1);assert.equal(warnings.length,new Set(warnings).size);
});
test('the two-retry scan limit is shared across batches including the initial page',async()=>{
  const calls=new Map<number,number>();const q=await scan(async n=>{const count=(calls.get(n)??0)+1;calls.set(n,count);if(count===1)throw new AppError('UPSTREAM_TIMEOUT','timeout',true);return page(n,4);});
  assert.equal(at(q,'coverage','retryAttempts'),2);assert.equal(at(q,'coverage','recoveredPageFetches'),2);assert.equal(at(q,'coverage','failedPageFetches'),1);assert.equal(calls.get(2),1);assert.equal(at(q,'coverage','status'),'partial');
});
test('rate limits, invalid payloads, access failures, non-transient failures and exhausted budgets are not retried',async()=>{
  for(const [code,transient,budget]of [['RATE_LIMITED',true,45_000],['UPSTREAM_INVALID',true,45_000],['HYPIXEL_AUTH_FAILED',false,45_000],['UPSTREAM_TOO_LARGE',false,45_000],['UPSTREAM_UNAVAILABLE',false,45_000],['UPSTREAM_TIMEOUT',true,10_000]] as [string,boolean,number][]){
    let calls=0;const q=await scan(async n=>{if(n){calls++;throw new AppError(code,'failure',transient);}return page(0,2);},budget);
    assert.equal(calls,1,code);assert.equal(at(q,'coverage','retryAttempts'),0,code);assert.equal(q.lowestBinCoins,null);
  }
});
test('an aborted scan does not issue a retry',async()=>{
  let calls=0;
  await assert.rejects(scan(async(_n,signal)=>{calls++;await new Promise<void>(resolve=>signal!.aborted?resolve():signal!.addEventListener('abort',()=>resolve(),{once:true}));throw new AppError('UPSTREAM_TIMEOUT','timeout',true);},100),(e:AppError)=>e.code==='UPSTREAM_TIMEOUT');
  assert.equal(calls,1);
});
test('HTTP 200 body aborts, reader failures and malformed JSON receive distinct safe codes',async()=>{
  const controller=new AbortController();const bodyAbort=new HypixelUpstream('',async()=>new Response(new ReadableStream({pull(c){controller.abort();c.error(new DOMException('private detail','AbortError'));}})));
  await assert.rejects(bodyAbort.auctionPage(0,controller.signal),(e:AppError)=>e.code==='UPSTREAM_TIMEOUT'&&!e.message.includes('private'));
  const bodyRead=new HypixelUpstream('',async()=>new Response(new ReadableStream({pull(c){c.error(new TypeError('private socket'));}})));
  await assert.rejects(bodyRead.auctionPage(0),(e:AppError)=>e.code==='UPSTREAM_UNAVAILABLE'&&!e.message.includes('private'));
  const invalid=new HypixelUpstream('',async()=>new Response('not JSON'));
  await assert.rejects(invalid.auctionPage(0),(e:AppError)=>e.code==='UPSTREAM_INVALID');
  const fetchAbort=new HypixelUpstream('',async()=>{throw new DOMException('private','AbortError');});
  await assert.rejects(fetchAbort.auctionPage(0,AbortSignal.abort()),(e:AppError)=>e.code==='UPSTREAM_TIMEOUT');
});
