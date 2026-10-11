import test from 'node:test';
import assert from 'node:assert/strict';
import {mapGameFields,projectOwnGame} from '../src/factual-fields.js';
import {HypixelUpstream} from '../src/upstream.js';
import {collectionProgress} from '../src/collection-progress.js';
import {RequestCache} from '../src/cache.js';
import {ProfileService} from '../src/service.js';
import {DemoUpstream,DEMO_UUID} from '../src/demo.js';
import {ClientObservations} from '../src/client-observations.js';
import {at,AppError,type JsonObject,type Upstream} from '../src/types.js';
import {mkdtemp,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';

test('future own game fields survive projection, while private identities never do',()=>{
 const source={new_system:{level:0,unlocked:false,rewards:[],missing:null},banking:{transactions:[{amount:5,initiator_name:'PrivatePlayer'}]},members:{['a'.repeat(32)]:{private:'secret'}},email:'a@example.com',encoded:{data:'H4sIAAAAAAA='}};
 const projected=projectOwnGame(source),m=mapGameFields(projected),fields=m.fields as JsonObject[];
 assert.ok(fields.some(f=>f.path==='member.new_system.level'&&f.value===0));assert.ok(fields.some(f=>f.path==='member.new_system.unlocked'&&f.value===false));assert.ok(fields.some(f=>f.path==='member.new_system.rewards'&&f.status==='known'));assert.ok(fields.some(f=>f.path==='member.new_system.missing'&&f.status==='unknown'));
 assert.equal(at(projected,'encoded','data'),'H4sIAAAAAAA=');assert.ok(!JSON.stringify(m).includes('PrivatePlayer'));assert.ok(!JSON.stringify(m).includes('H4sIAAAAAAA='));assert.ok(!JSON.stringify(m).includes('example.com'));assert.equal(at(m,'coverage','allSemanticsVerified'),false);
 const many=mapGameFields({containers:Array.from({length:20},()=>({data:'H4sIAAAAAAA='}))},'member','member',3);assert.equal((many.fields as unknown[]).length,3);assert.equal(at(many,'coverage','truncated'),true);
});
test('official collection thresholds do not replace unknown member or co-op amounts with zero',()=>{
 const resource={collections:{MINING:{items:{GOLD_INGOT:{name:'Gold',maxTiers:2,tiers:[{tier:1,amountRequired:50,unlocks:['Example reward']},{tier:2,amountRequired:100,unlocks:[]}]},DIAMOND:{name:'Diamond',maxTiers:1,tiers:[{tier:1,amountRequired:10}]}}}}};
 const result=collectionProgress({collection:{GOLD_INGOT:75},player_data:{unlocked_coll_tiers:['GOLD_INGOT_1']}},resource),rows=result.data.progress as JsonObject[];
 const gold=rows.find(r=>r.id==='GOLD_INGOT')!,diamond=rows.find(r=>r.id==='DIAMOND')!;
 assert.equal(gold.tierByMemberAmount,1);assert.equal(gold.remainingToNextTier,25);assert.equal(gold.coOpTier,null);assert.equal(gold.physicalResourcesAvailable,null);assert.equal(diamond.amount,null);assert.equal(diamond.status,'unknown');
});
test('numeric dungeon secrets survive private-field filtering but text credentials do not',()=>{
 const source={dungeons:{secret_count:27,secrets_found:0},new_system:{token_count:3},token:'private',secret_count:'private'};
 const fields=mapGameFields(projectOwnGame(source)).fields as JsonObject[];
 assert.equal(fields.find(f=>f.path==='member.dungeons.secret_count')?.value,27);
 assert.equal(fields.find(f=>f.path==='member.dungeons.secrets_found')?.value,0);
 assert.equal(fields.find(f=>f.path==='member.new_system.token_count')?.value,3);
 assert.ok(!JSON.stringify(fields).includes('private'));
});
test('supplemental endpoints exclude other members and other profiles; field timestamps identify the actual snapshot',async()=>{
 const other='f'.repeat(32),profile='1'.repeat(32),now=1800000000000;
 const up=new HypixelUpstream('fixture-key',async(input)=>{
  const url=new URL(String(input));
  if(url.pathname.endsWith('/museum'))return Response.json({success:true,members:{[DEMO_UUID]:{value:0,items:{}},[other]:{private:'other-member'}}});
  return Response.json({success:true,auctions:[{profile_id:profile,starting_bid:10,auctioneer:DEMO_UUID},{profile_id:other,starting_bid:999}]});
 });
 const museum=await up.supplement('museum',DEMO_UUID,profile),auctions=await up.supplement('auctions',DEMO_UUID,profile);
 assert.equal(at(museum,'ownMember','value'),0);assert.ok(!JSON.stringify(museum).includes('other-member'));
 assert.equal((auctions.auctions as JsonObject[]).length,1);assert.equal(at((auctions.auctions as JsonObject[])[0],'starting_bid'),10);assert.ok(!JSON.stringify(auctions).includes(DEMO_UUID));
 const demo:Upstream=new DemoUpstream(),cache=new RequestCache(':memory:',()=>now);
 demo.reference=async()=>({items:[{id:'EXAMPLE',name:'Example item'}],lastUpdated:now-100});
 demo.supplement=async()=>museum;
 try{
  const service=new ProfileService(demo,cache,()=>now),reference=await service.reference('items','EXAMPLE'),section=await service.query('DemoPlayer',undefined,'museum');
  assert.equal(at((reference.fields as JsonObject[])[0],'observedAt'),new Date(now).toISOString());
  assert.equal((at(section,'result','data','supplement','fields') as JsonObject[])[0]?.observedAt,new Date(now).toISOString());
  assert.equal(reference.upstreamLastUpdated,now-100);
 }finally{cache.close();}
});
test('cached snapshot reads survive TTL without fetching and require explicit refresh after expiry',async()=>{
 let now=1000,calls=0;const cache=new RequestCache(':memory:',()=>now),load=async()=>({retrievedAt:now,value:++calls});
 try{await cache.snapshot('player',100,1000,load,v=>v.retrievedAt,'cached');now+=200;const cached=await cache.snapshot('player',100,1000,load,v=>v.retrievedAt,'cached');assert.equal(cached.cache,'stale');assert.equal(calls,1);now+=1000;await assert.rejects(cache.snapshot('player',100,1000,load,v=>v.retrievedAt,'cached'),(e:AppError)=>e.code==='SNAPSHOT_REFRESH_REQUIRED');assert.equal(calls,1);await cache.snapshot('player',100,1000,load,v=>v.retrievedAt,'snapshot');assert.equal(calls,2);}finally{cache.close();}
});
test('API live mode is rejected before identity resolution or any other upstream call',async()=>{
 const up=new DemoUpstream(),cache=new RequestCache();let calls=0;up.resolve=async()=>{calls++;throw Error('must not fetch');};
 try{const service=new ProfileService(up,cache);await assert.rejects(service.query('Player','Apple','stats',{mode:'live'}),(e:AppError)=>e.code==='API_LIVE_POLLING_DISABLED');assert.equal(calls,0);const live=await service.clientContext('Player','Apple',undefined,true);assert.equal(live.status,'unavailable');assert.equal(calls,0);}finally{cache.close();}
});
test('live collector reads expose only currently observed owner fields, independently of API',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'bc-live-')),path=join(dir,'latest.json'),now=1800000000000;
 const item={slot:0,id:'NEX_TITANUM',count:1,enchantments:{},structuredAttributes:{fields:[{path:'upgrade_level',value:5,semanticStatus:'reported_attribute'}],omitted:0,truncated:false,complete:false}};
 const record={schemaVersion:1,collectorVersion:'0.3.0',exportedAt:now,activeProfile:DEMO_UUID+':apple',observedNow:['inventory'],exportIntervalMs:1000,profiles:[{uuid:DEMO_UUID,playerName:'Player',profileName:'Apple',observations:{inventory:{observedAt:now,readFrom:'player_inventory',values:{items:[item,item],observedSlots:41,completeStorage:false}},'bank:personalBankCoins':{observedAt:now,readFrom:'opened_menu',values:{personalBankCoins:500}}}}]};
 try{await writeFile(path,JSON.stringify(record),{mode:0o600});const client=new ClientObservations(path,()=>now),result=await client.query('Player','Apple',undefined,{live:true,limit:1});assert.equal(result.apiRequests,0);assert.equal(at(result,'observations','inventory','currentStateVerified'),true);assert.equal(at(result,'observations','bank:personalBankCoins'),undefined);assert.equal(at(result,'unavailableObservations','bank:personalBankCoins','reasonCode'),'not_currently_observed_or_too_old');assert.equal(at(result,'pagination','inventory.items','total'),2);assert.equal(at(result,'pagination','inventory.items','returned'),1);assert.ok(!JSON.stringify(result).includes('Bearer'));record.activeProfile=null as unknown as string;await writeFile(path,JSON.stringify(record));assert.equal((await client.query('Player','Apple',undefined,{live:true})).status,'unavailable');}finally{await rm(dir,{recursive:true,force:true});}
});

test('pinned node expressions and token balances never evaluate arbitrary code or missing counters',async()=>{
 const {ruleNumber,treeNodes}=await import('../src/system-definitions.js');
 const {gameSystems}=await import('../src/game-systems.js');
 assert.equal(ruleNumber('(pow (+ level 2) 3)',{level:20}),10648);assert.equal(ruleNumber('(if (lt level 20) (+ 10 (* level 0.5)) 30)',{level:20}),30);assert.equal(ruleNumber('(process.exit 1)',{}),null);assert.equal(ruleNumber('(/ 1 0)',{}),null);
 const nodes=treeNodes('mining',[['mining',{mining_speed:20,core_of_the_mountain:5}]],3);const speed=nodes.find(n=>n.id==='mining_speed')!;assert.equal(at(speed,'effects','stat'),400);assert.equal(at(speed,'nextLevelCost','amount'),10648);assert.ok(nodes.some(n=>n.level===null));
 const s=gameSystems({skill_tree:{experience:{mining:3000},nodes:{mining:{core_of_the_mountain:5}},tokens_spent:{mountain:4}},mining_core:{powder_mithril:0}},{});assert.equal(at(s.mining.data,'powders','0'),undefined);assert.equal(at(s.mining.data,'heartOfTheMountain','core','level','value'),5);assert.equal((s.mining.data.powders as JsonObject[])[0]!.type,'mithril');assert.equal(at((s.mining.data.powders as JsonObject[])[0],'available','value'),0);
});
test('client pet XP uses the same progression rules, and factual field pages are independent of item pages',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'bc-client-fields-')),path=join(dir,'latest.json'),now=1800000000000;
 const pet={slot:0,id:'PET',count:1,enchantments:{},structuredAttributes:{fields:Object.entries({type:'GOLDEN_DRAGON',tier:'LEGENDARY',exp:214023230,candyUsed:2,heldItem:'PET_ITEM_TIER_BOOST'}).map(([key,value])=>({path:'petInfo.'+key,value,semanticStatus:'reported_attribute'})),omitted:0,truncated:false,complete:false}};
 const data={schemaVersion:1,collectorVersion:'0.3.0',exportedAt:now,activeProfile:DEMO_UUID+':apple',observedNow:['inventory'],profiles:[{uuid:DEMO_UUID,playerName:'Player',profileName:'Apple',observations:{inventory:{observedAt:now,readFrom:'player_inventory',values:{items:[pet],observedSlots:41,completeStorage:false}}}}]};
 try{
  await writeFile(path,JSON.stringify(data),{mode:0o600});const client=new ClientObservations(path,()=>now);
  const first=await client.query('Player','Apple',undefined,{live:true,limit:1}),second=await client.query('Player','Apple',undefined,{live:true,offset:1,limit:1});
  const item=(at(first,'observations','inventory','values','items') as JsonObject[])[0]!;
  assert.equal(at(item,'pet','progression','level'),200);assert.equal(at(item,'pet','candiesUsed'),2);
  assert.equal(at(first,'pagination','fields','total'),at(second,'pagination','fields','total'));
  assert.equal((second.fields as unknown[]).length,1);assert.notEqual((first.fields as JsonObject[])[0]?.path,(second.fields as JsonObject[])[0]?.path);
  assert.ok((first.fields as JsonObject[]).every(f=>f.observedAt===new Date(now).toISOString()));assert.equal(first.apiRequests,0);
 }finally{await rm(dir,{recursive:true,force:true});}
});
