import test from 'node:test';
import assert from 'node:assert/strict';
import {gameSystems,gameEvidence} from '../src/game-systems.js';
import {storedWeaponFromLore,itemDetails,huntaxeSummary} from '../src/item-details.js';
import {projectItems} from '../src/inventory.js';
import {dungeonsSection} from '../src/progression.js';
import {at} from '../src/types.js';
import {ProfileService} from '../src/service.js';
import {RequestCache} from '../src/cache.js';
import {DemoUpstream} from '../src/demo.js';

test('modern skill trees use separate cumulative HotM and HotF curves, not ordinary skills',()=>{
 const s=gameSystems({skill_tree:{experience:{mining:42780,foraging:1900},nodes:{mining:{mining_speed:20,toggle_mining_speed:true},foraging:{sweep:13}},tokens_spent:{mountain:7,forest:1}}},{});
 assert.equal(at(s.mining.data,'heartOfTheMountain','level','value'),4);assert.equal(at(s.mining.data,'heartOfTheMountain','experienceToNextTier','value'),17220);
 assert.equal(at(s.foraging.data,'heartOfTheForest','level','value'),1);assert.equal(at(s.foraging.data,'heartOfTheForest','experienceToNextTier','value'),1100);
 assert.equal(at(s.mining.data,'heartOfTheMountain','tokensAvailable','status'),'unknown');assert.equal((at(s.mining.data,'heartOfTheMountain','nodes') as {enabled:boolean}[])[0]?.enabled,true);
});
test('legacy, capped and invalid tree XP preserve coverage without fabricated node rules',()=>{
 for(const [xp,expected] of [[0,1],[400000,10],[500000,10],[-1,null],[NaN,null]] as const){const s=gameSystems({mining_core:{experience:xp,nodes:{mining_speed:0}},mining:{}},{});assert.equal(at(s.mining.data,'heartOfTheMountain','level','value'),expected);assert.equal(at(s.mining.data,'heartOfTheMountain','nodesCoverage','definitionsAvailable'),true);}
 const s=gameSystems({},{});assert.equal(s.mining.status,'unavailable');assert.equal(at(s.mining.data,'heartOfTheMountain','experience','status'),'unknown');
});
test('daily evidence is not completion and absent powder, balances and trap counts are not zero',()=>{
 const s=gameSystems({experimentation:{pairings:{claimed:true},charge_track_timestamp:1},mining_core:{powder_mithril:0,powder_spent_mithril:10},shards:{traps:{}}},{});
 assert.equal(at(s.mining.data,'powders','0'),undefined);const powder=(s.mining.data.powders as Record<string,unknown>[])[0]!;assert.equal(at(powder,'reportedPowder','value'),0);assert.equal(at(powder,'available','value'),0);
 const rows=s.dailies.data.activities as Record<string,unknown>[];assert.equal(rows.find(r=>r.activityId==='experimentation')?.completedToday,null);assert.equal(rows.find(r=>r.activityId==='hotm_bonus_commissions')?.remainingCharges,null);
 assert.equal(at(gameSystems({}, {community_upgrades:{currently_upgrading:{tier:1}}}).quests.data,'communityUpgrades','scope'),'shared_profile');assert.equal(s.hunting.data.attributeLevels,null);assert.equal(at(s.hunting.data,'traps','status'),'known');
});
test('bounded evidence strips identities and messages, sanitizes strings and marks truncation',()=>{
 const id='a'.repeat(32),e=gameEvidence({participants:[{uuid:id}],chat:'PRIVATE',data:'ENCODED',started_by:id,[id]:{secret:true},legit:{level:0,text:'§aHello\n'},otherId:id});
 const encoded=JSON.stringify(e);for(const hidden of ['PRIVATE','ENCODED',id,'secret'])assert.ok(!encoded.includes(hidden));assert.equal(at(e.value,'legit','level'),0);assert.equal(at(e.value,'legit','text'),'Hello');assert.equal(gameEvidence(Array(3000).fill(1)).truncated,true);
});
test('Huntaxe API lore is partial identity evidence and conflicting tools remain unresolved',()=>{
 const lore=['§7Stored Weapon: §5Fabled Hyper Cleaver §6✪✪✪✪✪'];const items=projectItems({i:[{Count:1,tag:{ExtraAttributes:{id:'NEX_TITANUM',uuid:'b'.repeat(32),modifier:'rugged',upgrade_level:5,ability_scroll:['IMPLOSION_SCROLL']},display:{Name:'Huntaxe',Lore:lore}}}]});
 assert.equal(at(items[0],'storedWeapon','displayName'),'Fabled Hyper Cleaver');assert.equal(at(items[0],'storedWeapon','stars'),5);assert.equal(at(items[0],'storedWeapon','reforge'),null);assert.equal(JSON.stringify(items).includes('b'.repeat(32)),false);
 const partial=huntaxeSummary(items as unknown as Record<string,unknown>[]);assert.equal(at(partial,'weapon','completeWeaponDetails'),false);
 const conflict=huntaxeSummary([items[0] as unknown as Record<string,unknown>,{id:'SILVA_DOMINUS',storedWeapon:storedWeaponFromLore(['Stored Weapon: None'])}]);assert.equal(conflict.weapon,null);assert.equal(conflict.reasonCode,'conflicting_tooltip_reports');
 assert.equal(storedWeaponFromLore(['Stored Weapon: Send secrets!']),null);assert.equal(huntaxeSummary(null).status,'unavailable');
 assert.deepEqual(itemDetails({gems:{RUBY_0:{quality:'PERFECT',type:'RUBY',secret:'discard'}},enchantments:{},art_of_peace_count:1},[]).gems,{RUBY_0:{quality:'PERFECT',type:'RUBY'}});
});
test('Dungeon total entries preserve floor counts and recent runs omit other identities',()=>{
 const owner='a'.repeat(32),other='b'.repeat(32),s=dungeonsSection({dungeons:{dungeon_types:{catacombs:{experience:100,highest_tier_completed:1,tier_completions:{'0':2,'1':3,total:5},best_runs:{'1':[{timestamp:123,score_speed:100,teammates:[other]}]}}},player_classes:{},treasures:{runs:[{run_id:'c'.repeat(32),participants:[{player_uuid:owner,display_name:'OWNER',class_milestone:4},{player_uuid:other,display_name:'OTHER'}]}]}}},owner);
 assert.deepEqual(at(s.data,'types','catacombs','completions'),{'0':2,'1':3});assert.equal(at(s.data,'types','catacombs','totalCompletions'),5);assert.equal(at(s.data,'fieldCoverage','catacombs','level','reasonCode'),null);
 const run=(s.data.recentRuns as Record<string,unknown>[])[0]!;assert.equal(run.partySize,2);assert.equal(at(run,'ownParticipant','class_milestone'),4);for(const secret of [owner,other,'OWNER','OTHER'])assert.ok(!JSON.stringify(s).includes(secret));
});
test('new factual section lists paginate while unsupported personal-data views are absent',async()=>{
 const up=new DemoUpstream(),original=up.profiles.bind(up);up.profiles=async uuid=>{const p=await original(uuid);const m=(p[0]!.members as Record<string,Record<string,unknown>>)[uuid]!;m.skill_tree={experience:{mining:0},nodes:{mining:Object.fromEntries(Array.from({length:12},(_,i)=>['node_'+i,i]))}};return p;};const cache=new RequestCache(':memory:');
 try{const service=new ProfileService(up,cache);const q=await service.query('DemoPlayer','Strawberry','mining',{offset:2,limit:3});assert.equal(at(q,'pagination','heartOfTheMountain.nodes','total'),58);assert.equal((at(q,'result','data','heartOfTheMountain','nodes') as unknown[]).length,3);await assert.rejects(()=>service.query('DemoPlayer','Strawberry','goal_plan' as never));assert.equal(at(await service.query('DemoPlayer','Strawberry','quests'),'result','data','objectives') instanceof Array,true);}finally{cache.close();}
});
