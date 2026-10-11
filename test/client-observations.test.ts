import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,writeFile,chmod,symlink,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
import {ClientObservations} from '../src/client-observations.js';
import {ProfileService} from '../src/service.js';
import {RequestCache} from '../src/cache.js';
import {DemoUpstream,DEMO_UUID} from '../src/demo.js';
import {createHttp} from '../src/http.js';
import {Client,StreamableHTTPClientTransport} from '@modelcontextprotocol/client';
import {once} from 'node:events';
import type {AddressInfo} from 'node:net';
import {at} from '../src/types.js';
const time=1_800_000_000_000;
const weapon={slot:22,id:'HYPER_CLEAVER',count:1,reforge:'fabled',enchantments:{sharpness:6},hot_potato_count:10};
function data(observedAt=time){return {schemaVersion:1,collectorVersion:'0.1.0',exportedAt:time,activeProfile:DEMO_UUID+':strawberry',profiles:[{uuid:DEMO_UUID,playerName:'DemoPlayer',profileName:'Strawberry',observations:{huntaxe:{observedAt,readFrom:'opened_menu',values:{weapon:{...weapon,lore:'do not expose this',uuid:'secret-id'}}},'bank:personalBankCoins':{observedAt,readFrom:'opened_menu',values:{personalBankCoins:0}},cookie:{observedAt,readFrom:'tab_display',values:{active:true,remainingSecondsAtObservation:3600}},'storage:Jumbo Backpack':{observedAt,readFrom:'opened_menu',values:{items:[weapon],observedSlots:54,completeStorage:false}},ignored:{observedAt,readFrom:'opened_menu',values:{private:'must not appear'}}}}]};}
async function fixture(run:(path:string,reader:ClientObservations)=>Promise<void>){const dir=await mkdtemp(join(tmpdir(),'blockcontext-client-')),path=join(dir,'snapshot.json');try{await writeFile(path,JSON.stringify(data()),{mode:0o600});await run(path,new ClientObservations(path,()=>time));}finally{await rm(dir,{recursive:true,force:true});}}
test('reader accepts the corrected collector while rejecting unknown versions',async()=>fixture(async(path,reader)=>{
  for(const version of ['0.1.0','0.1.1','0.2.0','0.2.1','0.2.2','0.3.0','0.3.1']){const record=data();record.collectorVersion=version;await writeFile(path,JSON.stringify(record));assert.equal((await reader.query(DEMO_UUID,'Strawberry')).status,'partial');}
  const record=data();record.collectorVersion='9.9.9';await writeFile(path,JSON.stringify(record));assert.equal((await reader.query(DEMO_UUID,'Strawberry')).status,'unavailable');
}));
test('client data is owner/profile bound, projected and separated by section',async()=>fixture(async(path,reader)=>{
  const q=await reader.query(DEMO_UUID,'Strawberry','equipment');assert.equal(at(q,'observations','huntaxe','values','weapon','reforge'),'fabled');assert.equal(at(q,'observations','storage:Jumbo Backpack','values','completeStorage'),false);assert.equal(at(q,'observations','huntaxe','currentStateVerified'),false);assert.equal(at(q,'observations','cookie'),undefined);
  for(const secret of ['do not expose this','secret-id','must not appear'])assert.ok(!JSON.stringify(q).includes(secret));
  assert.equal((await reader.query('SomeoneElse','Strawberry')).reasonCode,'no_matching_player_and_profile');assert.equal((await reader.query(DEMO_UUID,'Watermelon')).status,'unavailable');
  assert.equal(at(await reader.query(DEMO_UUID,'Strawberry','currencies'),'observations','bank:personalBankCoins','values','personalBankCoins'),0);
}));
test('snapshots fail closed for symlinks, public permissions, malformed data and future/expired observations',async()=>fixture(async(path,reader)=>{
  if(process.platform!=='win32'){await chmod(path,0o644);assert.equal((await reader.query(DEMO_UUID)).status,'unavailable');await chmod(path,0o600);await symlink(path,path+'.link');assert.equal((await new ClientObservations(path+'.link',()=>time).query(DEMO_UUID)).status,'unavailable');}
  await writeFile(path,'invalid');assert.equal((await reader.query(DEMO_UUID)).status,'unavailable');
  await writeFile(path,JSON.stringify(data(time+1)));assert.equal((await reader.query(DEMO_UUID)).status,'unavailable');await writeFile(path,JSON.stringify(data(time-86400_000)));assert.equal((await reader.query(DEMO_UUID)).status,'unavailable');
}));
test('stale observations keep displayed timers unchanged and never certify connection or full coverage',async()=>fixture(async(path,reader)=>{
  const old=data(time-301000);old.exportedAt=time-301000;await writeFile(path,JSON.stringify(old));
  const q=await reader.query(DEMO_UUID,'Strawberry','stats');assert.equal(at(q,'observations','cookie','status'),'stale');assert.equal(at(q,'observations','cookie','values','remainingSecondsAtObservation'),3600);assert.equal(q.connectedAtLastExport,false);
  const newer=new ClientObservations(path,()=>time+400000);assert.equal(at(await newer.query(DEMO_UUID,'Strawberry','stats'),'observations','cookie','values','remainingSecondsAtObservation'),3600);
}));
test('MCP client tool works without API access and mixed profile output never replaces API values',async()=>fixture(async(path,reader)=>{
  const up=new DemoUpstream(),cache=new RequestCache(':memory:',()=>time),service=new ProfileService(up,cache,()=>time,reader),q=await service.query('DemoPlayer','Strawberry','currencies');assert.equal(at(q,'result','data','clientObservations','source'),'client_observation');assert.notEqual(at(q,'result','data','personalBankCoins'),0);
  up.resolve=async()=>{throw new Error('API must not run');};const server=createHttp(service,'demo');server.listen(0,'127.0.0.1');await once(server,'listening');const client=new Client({name:'collector-test',version:'1'});
  try{await client.connect(new StreamableHTTPClientTransport(new URL(`http://127.0.0.1:${(server.address() as AddressInfo).port}/mcp`)));const tools=await client.listTools();assert.ok(tools.tools.find(t=>t.name==='skyblock_client_context')?.annotations?.readOnlyHint);const r=await client.callTool({name:'skyblock_client_context',arguments:{player:'DemoPlayer',profile:'Strawberry',section:'equipment'}});assert.equal(r.isError,undefined);assert.equal(at(r.structuredContent,'observations','huntaxe','values','weapon','id'),'HYPER_CLEAVER');}finally{await client.close();server.close();server.closeAllConnections();await once(server,'close');cache.close();}
}));
test('expanded collector preserves inventory identity and typed menu facts without lore or hidden fields',async()=>fixture(async(path,reader)=>{
 const record=data();record.collectorVersion='0.2.0';const obs=record.profiles[0]!.observations as Record<string,unknown>;
 obs.inventory={observedAt:time,readFrom:'player_inventory',values:{items:[{...weapon,slot:0,instanceId:'f'.repeat(64),storedWeapon:{status:'partial',displayName:'Fabled Hyper Cleaver',stars:5,completeWeaponDetails:false},abilityScrolls:['IMPLOSION_SCROLL'],attributes:{warrior:2},gems:{RUBY_0:{quality:'PERFECT',type:'RUBY',secret:'discard'}},lore:'never export'}],observedSlots:41,completeStorage:false}};
 obs.mining={observedAt:time,readFrom:'opened_menu',values:{facts:[{slot:1,displayName:'Mining Speed',fields:[{label:'Level',value:20,maximum:50,unit:null,secret:'discard'}]}],observedSlots:54,completeSystem:false}};
 await writeFile(path,JSON.stringify(record));const gear=await reader.query(DEMO_UUID,'Strawberry','equipment');assert.equal(at(gear,'observations','inventory','readFrom'),'player_inventory');const items=at(gear,'observations','inventory','values','items') as Record<string,unknown>[];assert.equal(at(items[0],'storedWeapon','stars'),5);assert.equal(at(items[0],'attributes','warrior'),2);assert.equal(items[0]?.instanceId,'f'.repeat(64));assert.ok(!JSON.stringify(gear).includes('discard'));assert.ok(!JSON.stringify(gear).includes('never export'));
 const mining=await reader.query(DEMO_UUID,'Strawberry','mining');assert.equal(at(mining,'observations','mining','values','completeSystem'),false);assert.equal(at(mining,'observations','inventory'),undefined);
}));
test('unrecognized numeric facts and exaggerated completeness fail closed',async()=>fixture(async(path,reader)=>{
 const record=data();record.collectorVersion='0.2.0';const obs=record.profiles[0]!.observations as Record<string,unknown>;
 obs.mining={observedAt:time,readFrom:'opened_menu',values:{facts:[{slot:0,displayName:'Node',fields:[{label:'PRIVATE CHAT',value:1,maximum:null,unit:null}]}],observedSlots:54,completeSystem:true}};
 await writeFile(path,JSON.stringify(record));assert.equal((await reader.query(DEMO_UUID,'Strawberry','mining')).status,'unavailable');
}));
