import test from 'node:test';
import assert from 'node:assert/strict';
import { magicalPower } from '../src/accessories.js';
import { skillSection } from '../src/skills.js';
import { gardenSection, thresholdProgress } from '../src/garden.js';
import { MarketService } from '../src/market.js';
import { RequestCache } from '../src/cache.js';
import { DemoUpstream, DEMO_UUID } from '../src/demo.js';
import { HypixelUpstream } from '../src/upstream.js';
import { ProfileService } from '../src/service.js';
import { AppError, at, type JsonObject } from '../src/types.js';

const accessory = (id: string, rarity: string, slot = 0): JsonObject => ({ id, rarity, slot, category: 'ACCESSORY' });
test('magical power suppresses duplicate rarities and lower upgrade tiers', () => {
  const result = magicalPower([accessory('INTIMIDATION_TALISMAN','COMMON'), accessory('INTIMIDATION_RING','UNCOMMON',1), accessory('POTION_AFFINITY_TALISMAN','COMMON',2), accessory('ARTIFACT_POTION_AFFINITY','RARE',3), accessory('INTIMIDATION_RING','RARE',4)], {});
  assert.equal(result.currentMagicalPower, 16);
  assert.equal((result.activeItems as unknown[]).length, 2);
  assert.equal((result.suppressedItems as unknown[]).length, 3);
});
test('alias accessories contribute once, while special rarities use their own power', () => {
  const result = magicalPower([accessory('DANTE_TALISMAN','COMMON'), accessory('DANTE_RING','COMMON',1)], {});
  assert.equal(result.currentMagicalPower, 3);
  assert.equal((result.suppressedItems as unknown[]).length,1);
  assert.equal(magicalPower([accessory('DANTE_TALISMAN','SPECIAL')],{}).currentMagicalPower,3);
  assert.equal(magicalPower([accessory('DANTE_TALISMAN','VERY SPECIAL')],{}).currentMagicalPower,5);
});
test('Hegemony, active Abicase contacts and consumed Prism bonuses are additive', () => {
  const result = magicalPower([accessory('HEGEMONY_ARTIFACT','MYTHIC'),accessory('ABICASE','RARE',1),accessory('RIFT_PRISM','LEGENDARY',2)], { nether_island_player_data: { abiphone: { active_contacts: ['a','b','c','d','e','a'] } }, rift: { access: { consumed_prism: true } } });
  assert.equal(result.currentMagicalPower,65);
  assert.deepEqual(result.breakdown,{ accessories:52, abicaseContacts:2, consumedRiftPrism:11 });
  assert.equal(magicalPower([accessory('RIFT_PRISM','LEGENDARY')],{}).currentMagicalPower,0);
  assert.equal(magicalPower([], { rift:{ access:{ consumed_prism:true } } }).currentMagicalPower,11);
});
test('missing bags, unknown items and unverified bonus data cannot become complete totals', () => {
  assert.equal(magicalPower(null,{}).calculationStatus,'unavailable');
  for (const bag of [[accessory('FUTURE_ACCESSORY','RARE')],[accessory('INTIMIDATION_RING','UNKNOWN')],[accessory('ABICASE','RARE')],[accessory('INTIMIDATION_TALISMAN','MYTHIC'),accessory('INTIMIDATION_RING','UNCOMMON',1)]]) {
    const result=magicalPower(bag,{}); assert.equal(result.currentMagicalPower,null); assert.equal(result.calculationStatus,'partial');
  }
});

const catalog = { skills: Object.fromEntries([['FARMING',60],['TAMING',60],['RUNECRAFTING',25],['FORAGING',57],['MINING',60]].map(([name,max])=>[name,{maxLevel:max,levels:Array.from({length:Number(max)},(_,i)=>({level:i+1,totalExpRequired:(i+1)*100}))}])) };
function skill(name: string,xp: number,extra: JsonObject={},rank: JsonObject={}) { return at(skillSection({ ...extra, player_data:{experience:{[`SKILL_${name.toUpperCase()}`]:xp}} },catalog,rank).data,'skills',name); }
test('Farming purchased cap limits effective levels even when XP exceeds the cap', () => {
  const s=skill('farming',6050,{ jacobs_contest:{perks:{farming_level_cap:5}} });
  assert.equal(at(s,'xpLevel'),60); assert.equal(at(s,'playerLevelCap'),55); assert.equal(at(s,'effectiveLevelWithProgress'),55); assert.equal(at(s,'effectiveExperienceToNextLevel'),null); assert.equal(at(s,'atPlayerCap'),true);
  assert.equal(at(skill('farming',150,{jacobs_contest:{perks:{}}}),'playerLevelCap'),50);
  assert.equal(at(skill('farming',150,{jacobs_contest:{perks:null}}),'playerLevelCap'),null);
  assert.equal(at(skill('farming',150,{jacobs_contest:{perks:{farming_level_cap:11}}}),'playerLevelCap'),null);
});
test('George counts unique sacrifices and Runecrafting applies current rank gates', () => {
  assert.equal(at(skill('taming',5900,{pets_data:{pet_care:{pet_types_sacrificed:['SLUG','SLUG','SPIRIT']}}}),'playerLevelCap'),52);
  assert.equal(at(skill('taming',5900,{pets_data:{}}),'effectiveLevel'),50);
  assert.equal(at(skill('taming',5900,{pets_data:null}),'playerLevelCap'),null);
  assert.equal(at(skill('taming',5900,{pets_data:{pet_care:{pet_types_sacrificed:['']}}}),'playerLevelCap'),null);
  assert.equal(at(skill('runecrafting',1500,{}, {newPackageRank:'NONE'}),'effectiveLevel'),3);
  assert.equal(at(skill('runecrafting',1500,{}, {newPackageRank:'MVP_PLUS'}),'playerLevelCap'),25);
  assert.equal(at(skill('runecrafting',1500),'effectiveLevel'),null);
});
test('Foraging separates a known low level from unknown unlocks at and beyond level 50', () => {
  assert.equal(at(skill('foraging',1850),'effectiveLevelWithProgress'),18.5);
  assert.equal(at(skill('foraging',5050),'effectiveLevel'),50);
  assert.equal(at(skill('foraging',5050),'effectiveLevelWithProgress'),null);
  assert.equal(at(skill('foraging',5100),'effectiveLevel'),null);
  assert.equal(at(skill('foraging',5100),'playerLevelCapMaximum'),57);
  assert.equal(at(skill('mining',6050),'effectiveLevel'),60);
});

test('Garden XP and milestones use incremental thresholds including the initial zero threshold', () => {
  assert.deepEqual(thresholdProgress(10,[0,70,70]),{level:1,progress:1/7,toNext:60,maximumLevel:3});
  assert.equal(thresholdProgress(70,[0,70,70]).level,2);
  assert.equal(thresholdProgress(-1,[0,70]).level,null);
  const result=gardenSection({garden_experience:10,resources_collected:{WHEAT:73,POTATO_ITEM:100,CARROT_ITEM:100,'INK_SACK:3':100,NETHER_STALK:100},unlocked_plots_ids:['a','a','b'],commission_data:{visits:{},completed:{}}});
  assert.equal(result.status,'available'); assert.equal(result.data.unlockedPlotCount,2);
  const milestones=result.data.cropMilestones as JsonObject[];
  assert.equal(milestones[0]!.level,1); assert.equal(milestones[0]!.toNext,7);
  assert.deepEqual(milestones.map(v=>v.crop),['WHEAT','POTATO','CARROT','COCOA_BEANS','NETHER_WART']);
  assert.ok(milestones.every(v=>v.level !== null)); assert.equal(at(result.data,'visitors','totalVisits'),0);
});
test('Garden missing and invalid counters stay unknown, and composter remains a saved snapshot', () => {
  assert.equal(gardenSection(null).status,'unavailable');
  assert.equal(at(gardenSection({}).data,'visitors','totalVisits'),null);
  const result=gardenSection({garden_experience:-1,resources_collected:{WHEAT:-1},unlocked_plots_ids:[],composter_data:{conversion_ticks:300,last_save:123}});
  assert.equal(result.status,'partial'); assert.equal(result.data.experience,null); assert.equal(result.data.resourcesCollected,null);
  assert.equal(at(result.data,'composter','lastSavedAt'),123); assert.equal(at(result.data,'composter','organicMatter'),null);
});
test('Garden uses its own cached retrieval time and failure does not disable profile context', async () => {
  let now=1_000_000, calls=0; const cache=new RequestCache(':memory:',()=>now), upstream=new DemoUpstream();
  upstream.garden=async()=>{calls++; return {garden_experience:10,resources_collected:{WHEAT:73},unlocked_plots_ids:['a']};};
  try {
    const service=new ProfileService(upstream,cache,()=>now);
    await service.query('DemoPlayer','Strawberry','skills'); assert.equal(calls,0);
    now+=1000; const first=await service.query('DemoPlayer','Strawberry','garden');
    assert.notEqual(at(first,'metadata','retrievedAt'),at(first,'result','data','snapshot','retrievedAt'));
    await service.query('DemoPlayer','Strawberry'); assert.equal(calls,1);
    now+=1_800_001; upstream.garden=async()=>{throw new AppError('HYPIXEL_AUTH_FAILED','Access rejected.');};
    const context=await service.query('DemoPlayer','Strawberry'); assert.equal(at(context,'coverage','garden'),'unavailable'); assert.ok(at(context,'summary','skills'));
  } finally {cache.close();}
});

function marketSetup() {
  let now=1_000_000, calls=0, failure=false;
  const cache=new RequestCache(':memory:',()=>now), upstream=new DemoUpstream();
  upstream.itemCatalog=async()=>({lastUpdated:now,items:{WHEAT:{name:'Wheat',npcSellPrice:6},STONE:{name:'Stone',npcSellPrice:1}}});
  upstream.bazaar=async()=>{calls++; if(failure) throw new AppError('RATE_LIMITED','Throttled.',true); return {lastUpdated:now,products:{WHEAT:{buy_summary:[{amount:3,pricePerUnit:12},{amount:2,pricePerUnit:10}],sell_summary:[{amount:4,pricePerUnit:7},{amount:1,pricePerUnit:8}],quick_status:{buyPrice:99,sellPrice:1}}}};};
  return {cache,upstream,service:new MarketService(upstream,cache,()=>now),calls:()=>calls,advance:(n:number)=>{now+=n;},fail:()=>{failure=true;}};
}
test('Bazaar quotes walk asks upward and bids downward, with separate weighted summaries and NPC values', async()=>{
  const app=marketSetup(); try {
    const quote=await app.service.quote('wheat',4);
    assert.equal(at(quote,'bazaar','instantBuy','bestUnitPrice'),10); assert.equal(at(quote,'bazaar','instantBuy','totalCoins'),44);
    assert.equal(at(quote,'bazaar','instantSell','bestUnitPrice'),8); assert.equal(at(quote,'bazaar','instantSell','totalCoins'),29);
    assert.equal(at(quote,'bazaar','quickStatusWeightedPrices','buy'),99); assert.equal(at(quote,'npcSell','totalCoins'),24); assert.equal(quote.feesIncluded,false); assert.equal(quote.auctionPrice,null);
    await app.service.quote('WHEAT',1); assert.equal(app.calls(),1);
  } finally {app.cache.close();}
});
test('insufficient depth cannot invent a full quote; non-Bazaar items can still expose NPC value',async()=>{
  const app=marketSetup(); try {
    const quote=await app.service.quote('WHEAT',10); assert.equal(at(quote,'bazaar','instantBuy','totalCoins'),null); assert.equal(at(quote,'bazaar','instantBuy','filledQuantity'),5); assert.equal(at(quote,'bazaar','instantBuy','remainingQuantity'),5);
    const stone=await app.service.quote('STONE',3); assert.equal(at(stone,'bazaar','status'),'unavailable'); assert.equal(at(stone,'npcSell','totalCoins'),3);
    await assert.rejects(app.service.quote('UNKNOWN'),(e:AppError)=>e.code==='MARKET_ITEM_NOT_FOUND');
    for(const [id,q] of [['../WHEAT',1],['WHEAT',0],['WHEAT',1.5],['WHEAT',1_000_001]] as const) await assert.rejects(app.service.quote(id,q),(e:AppError)=>e.code==='INVALID_MARKET_QUERY');
  } finally {app.cache.close();}
});
test('market failures may use a briefly stale snapshot but cannot cross the upstream age limit',async()=>{
  const app=marketSetup(); try {
    await app.service.quote('WHEAT'); app.advance(46_000); app.fail();
    assert.equal(at(await app.service.quote('WHEAT'),'metadata','stale'),true);
    app.advance(134_000); await assert.rejects(app.service.quote('WHEAT'),(e:AppError)=>e.code==='RATE_LIMITED'||e.code==='MARKET_SNAPSHOT_EXPIRED');
  } finally {app.cache.close();}
});
test('malformed books and old or future upstream timestamps do not produce usable market prices',async()=>{
  for(const timestamp of [0,1_180_001]) {
    const app=marketSetup(); app.upstream.bazaar=async()=>({lastUpdated:timestamp,products:{}});
    try {await assert.rejects(app.service.quote('WHEAT'),(e:AppError)=>e.code==='MARKET_SNAPSHOT_EXPIRED');} finally{app.cache.close();}
  }
  const app=marketSetup(); app.upstream.bazaar=async()=>({lastUpdated:1_000_000,products:{WHEAT:{buy_summary:[{amount:-1,pricePerUnit:10}],sell_summary:[]}}});
  try {const quote=await app.service.quote('WHEAT'); assert.equal(at(quote,'bazaar','instantBuy','status'),'unavailable'); assert.equal(at(quote,'bazaar','instantSell','totalCoins'),null);}finally{app.cache.close();}
});
test('new upstream requests enforce identity, minimize rank data and keep public Bazaar requests unauthenticated',async()=>{
  let calls=0;
  const upstream=new HypixelUpstream('test-secret',async(input,init)=>{
    calls++; const url=new URL(String(input)); const header=new Headers(init?.headers).get('API-Key');
    if(url.pathname==='/v2/player') {assert.equal(header,'test-secret'); return Response.json({success:true,player:{uuid:DEMO_UUID,newPackageRank:'VIP',lastLogin:99,achievements:{secret:1}}});}
    if(url.pathname==='/v2/skyblock/garden') {assert.equal(header,'test-secret'); return Response.json({success:true,garden:{uuid:DEMO_UUID,garden_experience:10,unrelated:'discard'}});}
    assert.equal(url.pathname,'/v2/skyblock/bazaar'); assert.equal(header,null); return Response.json({success:true,lastUpdated:100,products:{}});
  });
  assert.deepEqual(await upstream.playerMetadata(DEMO_UUID),{rank:null,packageRank:null,newPackageRank:'VIP',monthlyPackageRank:null,skyblock:{},achievements:{},achievementsOneTime:null});
  assert.deepEqual(await upstream.garden(DEMO_UUID),{garden_experience:10,unrelated:'discard'}); await upstream.bazaar(); assert.equal(calls,3);
  await assert.rejects(upstream.garden('../bad')); await assert.rejects(upstream.playerMetadata('bad')); assert.equal(calls,3);
  const missing=new HypixelUpstream('test',async()=>new Response('',{status:404})); assert.equal(await missing.garden(DEMO_UUID),null);
  const wrong=new HypixelUpstream('test',async()=>Response.json({success:true,garden:{uuid:'f'.repeat(32),garden_experience:10}})); await assert.rejects(wrong.garden(DEMO_UUID),(e:AppError)=>e.code==='UPSTREAM_INVALID');
});
