import test from 'node:test';
import assert from 'node:assert/strict';
import {gzipSync} from 'node:zlib';
import {writeUncompressed,type NBT} from 'prismarine-nbt';
import {petProgress,dungeonProgress,dungeonsSection,petsSection} from '../src/progression.js';
import {loadouts} from '../src/loadouts.js';
import {AuctionService} from '../src/auctions.js';
import {RequestCache} from '../src/cache.js';
import {DemoUpstream,DEMO_UUID} from '../src/demo.js';
import {HypixelUpstream} from '../src/upstream.js';
import {ProfileService} from '../src/service.js';
import {at,object,type JsonObject,AppError} from '../src/types.js';
import {createHttp} from '../src/http.js';
import {Client,StreamableHTTPClientTransport} from '@modelcontextprotocol/client';
import {once} from 'node:events';
import type {AddressInfo} from 'node:net';

const SCROLLS=['IMPLOSION_SCROLL','SHADOW_WARP_SCROLL','WITHER_SHIELD_SCROLL'];
function encoded(id:string|null,extra:JsonObject={},count=1) {
  const fields:JsonObject={};for(const [k,v]of Object.entries(extra))fields[k]=Array.isArray(v)?{type:'list',value:{type:'string',value:v}}:{type:typeof v==='number'?'int':'string',value:v};
  const items=id===null?[]:[{Count:{type:'byte',value:count},tag:{type:'compound',value:{ExtraAttributes:{type:'compound',value:{id:{type:'string',value:id},...fields}}}}}];
  return gzipSync(writeUncompressed({type:'compound',name:'',value:{i:{type:'list',value:{type:'compound',value:items}}}} as unknown as NBT)).toString('base64');
}
function auctions(rows:{id:string;price:number;extra?:JsonObject}[]) {
  const now=1_000_000,up=new DemoUpstream(),cache=new RequestCache(':memory:',()=>now);
  up.auctionPage=async()=>({page:0,totalPages:1,totalAuctions:rows.length,lastUpdated:now,auctions:rows.map((r,i)=>({uuid:(i+1).toString(16).padStart(32,'0'),bin:true,claimed:false,end:now+120_000,starting_bid:r.price,tier:'LEGENDARY',item_bytes:encoded(r.id,r.extra)}))});
  return {cache,up,service:new AuctionService(up,cache,()=>now)};
}
test('pet XP uses rarity offsets, exact cap boundaries, dragon extension and explicit unknown curves',()=>{
  assert.equal(petProgress('SHEEP','COMMON',99).level,1);assert.equal(petProgress('SHEEP','COMMON',100).level,2);
  assert.equal(petProgress('SHEEP','LEGENDARY',25353229).level,99);assert.equal(petProgress('SHEEP','LEGENDARY',25353230).level,100);
  assert.equal(petProgress('BINGO','RARE',100).level,2);
  const egg=petProgress('GOLDEN_DRAGON','LEGENDARY',25353229);assert.equal(egg.dragonStage,'egg');
  const hatched=petProgress('GOLDEN_DRAGON','LEGENDARY',25353230);assert.equal(hatched.level,100);assert.equal(hatched.dragonStage,'hatched');assert.equal(hatched.experienceToMaxLevel,188670000);
  assert.equal(petProgress('GOLDEN_DRAGON','LEGENDARY',214023229).level,199);
  const max=petProgress('GOLDEN_DRAGON','LEGENDARY',214023230);assert.equal(max.level,200);assert.equal(max.experienceToNextLevel,null);assert.equal(max.experienceToMaxLevel,0);assert.equal(max.progressToMaxLevel,1);
  assert.equal(petProgress('GOLDEN_DRAGON','LEGENDARY',214023231).overflowExperience,1);
  for(const [type,tier,xp]of [['NEW_PET','LEGENDARY',1],['SHEEP','NEW_RARITY',1],['SHEEP','COMMON',-1],['SHEEP','COMMON',NaN],['SHEEP','COMMON',undefined]])assert.equal(petProgress(type,tier,xp).level,null);
  assert.equal(petProgress('REINDEER','LEGENDARY',660).level,2,'XP gain multiplier must not be reapplied');
});
test('Catacombs levels stop at 50 and distinguish next level from floor prerequisites',()=>{
  assert.equal(dungeonProgress(0).effectiveLevel,0);assert.equal(dungeonProgress(49).effectiveLevel,0);assert.equal(dungeonProgress(50).effectiveLevel,1);
  assert.equal(dungeonProgress(569809639).effectiveLevel,49);assert.equal(dungeonProgress(569809640).effectiveLevel,50);assert.equal(dungeonProgress(569809650).overflowExperience,10);
  assert.equal(dungeonProgress(-1).effectiveLevel,null);
  const data=dungeonsSection({dungeons:{dungeon_types:{catacombs:{experience:50,tier_completions:{'0':1}}},player_classes:{mage:{experience:50}}}});
  assert.equal(at(data,'data','types','catacombs','nextUnlock','floor'),2);assert.equal(at(data,'data','types','catacombs','nextUnlock','otherPrerequisitesAssessed'),false);
  assert.equal(dungeonsSection({dungeons:{dungeon_types:{catacombs:{tier_completions:{'0':1}}}}}).status,'partial');
  const invalid=dungeonsSection({dungeons:{dungeon_types:{catacombs:{experience:0,tier_completions:{'1':-1}}},player_classes:{}}});assert.equal(at(invalid,'data','types','catacombs','completions'),null);assert.equal(invalid.status,'partial');
  assert.equal(dungeonsSection({}).status,'unavailable');
});
test('auction filters separate bare, partially and fully scrolled blades and never accept unknown variants as complete',async()=>{
  const app=auctions([{id:'HYPERION',price:10},{id:'HYPERION',price:20,extra:{ability_scroll:['WITHER_SHIELD_SCROLL']}},{id:'HYPERION',price:30,extra:{ability_scroll:[...SCROLLS].reverse()}},{id:'HYPERION',price:1,extra:{ability_scroll:['UNKNOWN_SCROLL']}}]);
  try{
    const bare=await app.service.quote('HYPERION',{scrolls:[]});assert.equal(bare.lowestObservedBinCoins,10);assert.equal(bare.lowestBinCoins,null);assert.equal(at(bare,'coverage','unknownVariantListingCount'),1);assert.equal(at(bare,'coverage','scanStatus'),'complete');
    const full=await app.service.quote('HYPERION',{scrolls:SCROLLS});assert.equal(full.lowestObservedBinCoins,30);assert.equal(at((full.listings as JsonObject[])[0],'fullyScrolled'),true);
    const all=await app.service.quote('HYPERION');assert.equal(all.lowestBinCoins,1,'without variant filters all valid item asking prices are known');
    for(const [item,filters]of [['SWORD',{scrolls:[]}],['HYPERION',{scrolls:['IMPLOSION_SCROLL','IMPLOSION_SCROLL']}],['HYPERION',{scrolls:['UNKNOWN']}],['PET',{petLevel:200.1}],['SWORD',{petLevel:200}] ] as [string,JsonObject][])await assert.rejects(app.service.quote(item,filters),(e:AppError)=>e.code==='INVALID_AUCTION_QUERY');
  }finally{app.cache.close();}
  const clean=auctions([{id:'HYPERION',price:1},{id:'HYPERION',price:2,extra:{ability_scroll:SCROLLS}}]);try{assert.equal((await clean.service.quote('HYPERION',{scrolls:SCROLLS})).lowestBinCoins,2);}finally{clean.cache.close();}
});
test('auction pet level uses underlying pet rarity, keeps held items and skins safe, and excludes unknown XP',async()=>{
  const app=auctions([{id:'PET',price:1,extra:{petInfo:JSON.stringify({type:'GOLDEN_DRAGON',tier:'LEGENDARY',exp:0})}},{id:'PET',price:2,extra:{petInfo:JSON.stringify({type:'GOLDEN_DRAGON',tier:'LEGENDARY',exp:214023230,heldItem:'PET_ITEM_TIER_BOOST',skin:'GOLDEN_DRAGON_SKIN',uuid:'private-pet-uuid'})}},{id:'PET',price:0.5,extra:{petInfo:JSON.stringify({type:'GOLDEN_DRAGON',tier:'LEGENDARY'})}}]);
  try{const q=await app.service.quote('PET',{petType:'GOLDEN_DRAGON',petLevel:200});assert.equal(q.matchedListingCount,1);assert.equal(q.lowestObservedBinCoins,2);assert.equal(q.lowestBinCoins,null);assert.equal(at(q,'coverage','variantStatus'),'partial');const listing=(q.listings as JsonObject[])[0]!;assert.equal(listing.petLevel,200);assert.equal(listing.petRarity,'LEGENDARY');assert.equal(listing.petHeldItem,'PET_ITEM_TIER_BOOST');assert.ok(!JSON.stringify(q).includes('private-pet-uuid'));}finally{app.cache.close();}
});
test('modern loadouts decode one item per piece, preserve sets, and distinguish empty sets from malformed storage',async()=>{
  const result=await loadouts({'1':{id:1,HELMET:{data:encoded('HELMET')},BOOTS:{data:encoded('BOOTS')}},'2':{id:2},equipped_set:1},'armor');
  assert.equal(result.status,'available');assert.deepEqual(result.items!.map(v=>[v.id,v.setId,v.piece]),[['HELMET','1','HELMET'],['BOOTS','1','BOOTS']]);assert.equal(result.sets![1]!.itemCount,0);
  const partial=await loadouts({'1':{id:1,HELMET:{data:encoded('HELMET')},BOOTS:{data:'bad'}}},'armor');assert.equal(partial.status,'partial');assert.equal(partial.sets![0]!.itemCount,null);assert.equal(partial.items!.length,1);
  assert.equal((await loadouts({},'equipment')).status,'available');assert.equal((await loadouts(undefined,'armor')).reasonCode,'not_exposed');assert.equal((await loadouts([], 'armor')).status,'unavailable');
  assert.equal((await loadouts({'1':{id:2}},'armor')).status,'unavailable');assert.equal((await loadouts({'1':{id:1,HELMET:{data:encoded('HELMET',{},2)}}},'armor')).status,'unavailable');
  assert.equal((await loadouts(Object.fromEntries(Array.from({length:37},(_,i)=>[String(i),{id:i}])),'armor')).status,'unavailable');assert.equal((await loadouts({'1':{HELMET:{data:'A'.repeat(9*1024*1024)}}},'armor')).reasonCode,'safety_limit');
});
test('live upstream projection retains own loadout and personal bank while removing private notices',async()=>{
  const raw=(await new DemoUpstream().profiles(DEMO_UUID))[0]!,member=object(at(raw,'members',DEMO_UUID));
  member.loadout={armor:{'1':{id:1,HELMET:{data:encoded('HELMET')}}},equipment:{'1':{id:1}},loadouts:{'1':{name:'private setup'}}};member.profile={bank_account:123,deletion_notice:'private notice'};
  const up=new HypixelUpstream('test-key',async()=>new Response(JSON.stringify({success:true,profiles:[raw]})));const projected=(await up.profiles(DEMO_UUID))[0]!;
  assert.equal(at(projected,'members',DEMO_UUID,'profile','bank_account'),123);assert.equal(at(projected,'members',DEMO_UUID,'profile','deletion_notice'),undefined);assert.deepEqual(at(projected,'members',DEMO_UUID,'loadout','loadouts'),{'1':{name:'private setup'}});
  const demo=new DemoUpstream();demo.profiles=async()=>[projected];const cache=new RequestCache();try{
    const service=new ProfileService(demo,cache),currencies=await service.query('DemoPlayer','Strawberry','currencies');assert.equal(at(currencies,'result','data','personalBankCoins'),123);
    const gear=await service.query('DemoPlayer','Strawberry','equipment',{limit:1});assert.equal(at(gear,'pagination','wardrobeItems','total'),1);assert.equal(at(gear,'result','data','fieldCoverage','wardrobeItems','source'),'loadout.armor');assert.equal((at(gear,'result','data','wardrobeItems')as JsonObject[])[0]!.setId,'1');
  }finally{cache.close();}
});
test('known zero currency remains zero, missing storage leaves equipment partial, and malformed pets never imply an empty list',async()=>{
  const up=new DemoUpstream(),raw=(await up.profiles(DEMO_UUID))[0]!,m=object(at(raw,'members',DEMO_UUID));raw.banking=undefined;m.profile={bank_account:0};m.currencies={coin_purse:0};m.inventory={inv_armor:{data:encoded('HELMET')}};up.profiles=async()=>[raw];
  const cache=new RequestCache();try{const service=new ProfileService(up,cache),q=await service.query('DemoPlayer','Strawberry','currencies');assert.equal(at(q,'result','data','personalBankCoins'),0);assert.equal(at(q,'result','data','sharedProfileBankCoins'),null);assert.equal(at(q,'result','status'),'partial');assert.equal(at(q,'result','data','fieldCoverage','personalBankCoins','status'),'available');const gear=await service.query('DemoPlayer','Strawberry','equipment');assert.equal(at(gear,'result','status'),'partial');assert.equal(at(gear,'result','data','fieldCoverage','backpackItems','reasonCode'),'not_exposed');}finally{cache.close();}
  assert.equal(petsSection([{}]).status,'partial');assert.equal(at(petsSection({}),'data','pets'),null);assert.equal(petsSection([]).status,'available');
});
test('new auction filters are discoverable and callable through a real HTTP MCP client',async()=>{
  const app=auctions([{id:'HYPERION',price:20,extra:{ability_scroll:SCROLLS}}]),server=createHttp(new ProfileService(app.up,app.cache,()=>1_000_000),'demo');server.listen(0,'127.0.0.1');await once(server,'listening');const client=new Client({name:'variant-test',version:'1'});
  try{await client.connect(new StreamableHTTPClientTransport(new URL(`http://127.0.0.1:${(server.address() as AddressInfo).port}/mcp`)));const tools=await client.listTools(),schema=tools.tools.find(t=>t.name==='skyblock_auction_quote')!.inputSchema;assert.ok(at(schema,'properties','scrolls'));assert.ok(at(schema,'properties','petLevel'));const q=await client.callTool({name:'skyblock_auction_quote',arguments:{item:'HYPERION',scrolls:SCROLLS}});assert.ok(!q.isError);assert.equal(at(q.structuredContent,'lowestBinCoins'),20);const invalid=await client.callTool({name:'skyblock_auction_quote',arguments:{item:'PET',petLevel:201}});assert.equal(invalid.isError,true);}finally{await client.close();server.close();server.closeAllConnections();await once(server,'close');app.cache.close();}
});
test('legacy wardrobe works when modern armor is absent; malformed modern loadouts cannot masquerade as legacy emptiness',async()=>{
  const up=new DemoUpstream(),raw=(await up.profiles(DEMO_UUID))[0]!,m=object(at(raw,'members',DEMO_UUID));m.inventory={wardrobe_contents:{data:encoded('LEGACY_HELMET')}};up.profiles=async()=>[raw];
  for(const modern of [undefined,null,{armor:[]}] ){
    m.loadout=modern;const cache=new RequestCache();try{const q=await new ProfileService(up,cache).query('DemoPlayer','Strawberry','equipment');assert.equal(at(q,'result','data','fieldCoverage','wardrobeItems','source'),modern===undefined?'inventory.wardrobe_contents':'loadout.armor');assert.equal(at(q,'result','data','wardrobeItemCount'),modern===undefined?1:null);}finally{cache.close();}
  }
});
