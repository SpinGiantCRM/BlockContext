import test from 'node:test';
import assert from 'node:assert/strict';
import {gameSystems} from '../src/game-systems.js';
import {mapGameFields,fieldUnit} from '../src/factual-fields.js';
import {whisperBalances,withDisplayedWhispers} from '../src/whispers.js';
import {reconcileDailyActivities,observedActivities} from '../src/reported-progression.js';
import {ProfileService} from '../src/service.js';
import {RequestCache} from '../src/cache.js';
import {DemoUpstream} from '../src/demo.js';
import {ClientObservations} from '../src/client-observations.js';
import {at,type JsonObject} from '../src/types.js';
import {mkdtemp,writeFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';

test('null and missing evidence stay unknown; zero, false and empty are known',()=>{
 for(const value of [undefined,null]){
  const s=gameSystems({experimentation:value},{}),activities=s.dailies.data.activities as JsonObject[];
  assert.equal(at(activities.find(r=>r.activityId==='experimentation'),'evidence','status'),'unknown');
  assert.equal(at(activities[0],'evidence','status'),'unknown');assert.equal(at(activities[0],'evidence','value'),null);
 }
 for(const value of [0,false,{},[]])assert.equal(at(gameSystems({mining_core:{crystals:value}},{}).mining.data,'crystals','status'),'known');
 assert.equal(at(gameSystems({mining_core:{crystals:Array(3000).fill(1)}},{}).mining.data,'crystals','status'),'partial');
});
test('Experience enchantment is a level across API and client item mappings',()=>{
 for(const prefix of ['member.inventory.item','client.huntaxe.weapon']){
  const fields=mapGameFields({enchantments:{experience:3,sharpness:5},pet:{experience:123}},prefix).fields as JsonObject[];
  assert.equal(fields.find(f=>String(f.path).endsWith('enchantments.experience'))?.unit,'enchantment level');
  assert.equal(fields.find(f=>String(f.path).endsWith('pet.experience'))?.unit,'XP');
 }
 assert.equal(fieldUnit('member.skill_tree.experience.mining'),null);
});
test('modern Whispers expose totals and each preset without guessing missing counters or active preset',()=>{
 const member={foraging_core:{whispers:{forest:{total:13000,'1':{spent:11000},'2':{spent:7000,spent_non_refundable:100}},desert:{'1':{}}}}};
 const w=whisperBalances(member);assert.equal(at(w,'total','value'),13000);assert.equal(at(w,'available','status'),'unknown');assert.equal(at(w,'spent','status'),'unknown');
 const presets=at(w,'forest','presets') as JsonObject[];assert.equal(at(presets[0],'spent','value'),11000);assert.equal(at(presets[0],'spentNonRefundable','status'),'unknown');assert.equal(at(presets[1],'spentNonRefundable','value'),100);
 assert.equal(at(whisperBalances({...member,skill_tree:{selected_skill_tree_slot:{foraging:2}}}),'spent','value'),7000);
 assert.equal(at(whisperBalances({foraging_core:{forests_whispers:0,forests_whispers_spent:100}}),'available','value'),0);
 assert.equal(at(whisperBalances({foraging_core:{forests_whispers:0,forests_whispers_spent:100}}),'total','value'),100);
});
function whisperReport(value:number,current=true):JsonObject{return {observedAt:'2026-10-11T00:00:00Z',currentStateVerified:current,values:{facts:[{fields:[{label:'Forest Whispers',value}]}]}};}
test('Whisper display reconciliation keeps API arithmetic unverified and ignores stale displays',()=>{
 const api=whisperBalances({foraging_core:{whispers:{forest:{total:13000,'1':{spent:11000}}}}});
 assert.equal(at(withDisplayedWhispers(api,{observations:{foraging:whisperReport(2000)}}),'available','value'),2000);
 assert.equal(at(withDisplayedWhispers(api,{observations:{foraging:whisperReport(2000,false)}}),'available','status'),'unknown');
 assert.equal(at(withDisplayedWhispers(api,{observations:{foraging:whisperReport(2000),tabStats:whisperReport(1900)}}),'available','status'),'unknown');
 assert.equal(at(withDisplayedWhispers(api,{observations:{foraging:whisperReport(2000)}}),'reconciliation','crossSourceEqualityVerified'),false);
});
test('daily commissions require current explicit evidence; bonus presence is not a count or reset',()=>{
 const activities=gameSystems({},{}).dailies.data.activities as JsonObject[],view=(values:JsonObject,current=true)=>reconcileDailyActivities(activities,{observations:{commissions:{values,currentStateVerified:current,observedAt:'2026-10-11T00:00:00Z'}}})[0]!;
 const remaining=view({remainingBonusCommissions:0});assert.equal(remaining.completedToday,true);assert.equal(remaining.remainingCharges,0);assert.equal(remaining.usedCharges,null);assert.equal(remaining.nextReset,null);
 const marker=view({dailyBonusAvailable:true});assert.equal(marker.completedToday,false);assert.equal(marker.remainingCharges,null);assert.equal(marker.status,'partial');
 const stale=view({remainingBonusCommissions:0},false);assert.equal(stale.completedToday,null);assert.equal(stale.remainingCharges,null);assert.equal(stale.status,'unknown');assert.equal(at(stale,'lastObservation','remainingCharges'),0);
 for(const value of [-1,0.5,NaN,101])assert.equal(view({remainingBonusCommissions:value}).remainingCharges,null);
 assert.equal(observedActivities({observations:{mining:{values:{facts:[{fields:[{label:'Remaining Charges',value:-1}]}]}}}}).length,0);
});
test('contract Garden covers the same shared endpoint as overview, with a separate member snapshot and cached policy',async()=>{
 let now=1000000,calls=0;const up=new DemoUpstream(),cache=new RequestCache(':memory:',()=>now);
 up.garden=async()=>{calls++;return {garden_experience:10,unlocked_plots_ids:['a'],resources_collected:{WHEAT:73}};};
 try{
  const service=new ProfileService(up,cache,()=>now);await service.query('DemoPlayer','Strawberry','skills');now+=1000;
  const contract=await service.query('DemoPlayer','Strawberry','data_contract',{limit:100}),overview=await service.query('DemoPlayer','Strawberry');
  const rows=at(contract,'result','data','requirements') as JsonObject[];
  // Contract requirements are paged: Garden falls on its second page.
  const page=await service.query('DemoPlayer','Strawberry','data_contract',{offset:100,limit:100});rows.push(...at(page,'result','data','requirements') as JsonObject[]);
  const garden=rows.find(r=>r.id==='garden.level_and_plots')!;
  assert.equal(at(garden,'api','section'),at(overview,'coverage','garden'));assert.ok(Number(at(garden,'api','fieldCount'))>0);assert.equal(at(garden,'api','sourceCoverage','gardenEndpoint','scope'),'shared_profile');
  assert.notEqual(at(garden,'api','sourceCoverage','gardenEndpoint','snapshot','retrievedAt'),at(garden,'api','sourceCoverage','greenhouse','snapshot','retrievedAt'));assert.equal(calls,1);
  now+=300001;await service.query('DemoPlayer','Strawberry','data_contract',{mode:'cached'});assert.equal(calls,1);
 }finally{cache.close();}
});
test('commission observations survive validation and dailies filtering, independently of API',async()=>{
 const dir=await mkdtemp(join(tmpdir(),'bc-commissions-')),path=join(dir,'latest.json'),now=1800000000000,uuid='0123456789abcdef0123456789abcdef';
 const record={schemaVersion:1,collectorVersion:'0.3.1',exportedAt:now,activeProfile:uuid+':apple',observedNow:['commissions'],profiles:[{uuid,playerName:'Player',profileName:'Apple',observations:{commissions:{observedAt:now,readFrom:'opened_menu',values:{remainingBonusCommissions:2,nextResetSecondsAtObservation:100,private:'discard'}}}}]};
 try{
  await writeFile(path,JSON.stringify(record),{mode:0o600});const client=new ClientObservations(path,()=>now),response=await client.query('Player','Apple','dailies',{live:true});
  assert.equal(at(response,'observations','commissions','currentStateVerified'),true);assert.equal(at(response,'observations','commissions','values','remainingBonusCommissions'),2);assert.equal(response.apiRequests,0);assert.ok(!JSON.stringify(response).includes('discard'));
  record.profiles[0]!.observations.commissions.values.remainingBonusCommissions=-1;await writeFile(path,JSON.stringify(record));assert.equal((await client.query('Player','Apple','dailies',{live:true})).status,'unavailable');
 }finally{await rm(dir,{recursive:true,force:true});}
});
