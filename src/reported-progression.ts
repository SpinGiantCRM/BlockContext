import {readFileSync} from 'node:fs';
import {definitions} from './system-definitions.js';
import {at,number,object,text,type JsonObject,type Section} from './types.js';
const rules=JSON.parse(readFileSync(new URL('../../fixtures/domain/skill-trees.json',import.meta.url),'utf8')) as {slayerThresholds:Record<string,number[]>;counterRulesRevision:string};
export function slayerProgress(m:JsonObject):JsonObject {
 const raw=object(at(m,'slayer','slayer_bosses'));
 return Object.fromEntries(Object.entries(raw).map(([id,v])=>{
  const reported=number(at(v,'xp')),xp=reported!==null&&reported>=0?reported:null,curve=rules.slayerThresholds[id];
  const level=xp!==null&&curve?curve.filter(x=>xp>=x).length:null,next=xp!==null&&curve?curve.find(x=>x>xp)??null:null;
  return [id,{experience:xp,level,cap:curve?.length??null,experienceToNextLevel:xp!==null&&next!==null?next-xp:null,nextLevelThreshold:next,killsByTier:Object.fromEntries(Object.entries(object(v)).filter(([k,n])=>/^boss_kills_tier_\d+$/.test(k)&&number(n)!==null)),reportedClaimedLevels:at(v,'claimed_levels')??null,unlockedBossTiers:null,rewardClaimsVerified:false,reasonCode:curve?'boss_unlock_and_reward_rules_not_verified':'family_level_curve_not_verified',rules:{source:'https://github.com/SkyCryptWebsite/SkyCrypt-Backend',revision:rules.counterRulesRevision,liveGameRulesVerified:false}}];
 }));
}
export function bestiaryProgress(m:JsonObject):Section {
 const raw=object(at(m,'bestiary','kills')),defs=object(definitions.bestiary),refs=Array.isArray(defs.mobs)?defs.mobs as JsonObject[]:[];
 const families=refs.map(ref=>{
  const ids=Array.isArray(ref.ids)?ref.ids as string[]:[],counts=ids.map(id=>({id,kills:number(raw[id])})),complete=counts.length>0&&counts.every(x=>x.kills!==null&&x.kills>=0),knownKills=counts.reduce((sum,x)=>sum+(x.kills!==null&&x.kills>=0?x.kills:0),0),kills=complete?knownKills:null;
  const cap=number(ref.cap),bracket=at(defs,'brackets',String(ref.bracket)),tiers=Array.isArray(bracket)?bracket.filter((v):v is number=>typeof v==='number'&&cap!==null&&v<=cap):[],level=kills!==null&&tiers.length?tiers.filter(n=>kills>=n).length:null,next=kills!==null?tiers.find(n=>n>kills)??null:null;
  return {...ref,kills,knownKillsSubtotal:knownKills,tier:level,nextTierThreshold:next,remainingToNextTier:kills!==null&&next!==null?next-kills:null,counts,status:complete?'known':knownKills>0?'partial':'unknown',reasonCode:complete?null:'one_or_more_member_kill_counters_not_exposed',rewardClaims:null};
 });
 return {status:m.bestiary===undefined?'unavailable':'partial',data:{families,reportedKills:raw,coverage:{referenceFamilies:refs.length,returned:families.length,fullyKnown:families.filter(r=>r.status==='known').length,complete:false},rules:{source:definitions.source,revision:definitions.revision,liveGameRulesVerified:false}},warnings:['Missing mob counters remain unknown. Family thresholds use pinned community data; reward claims and effective combat bonuses are not inferred.']};
}
/** Only explicit currently displayed remaining-charge counters establish daily completion. */
export function observedActivities(client:JsonObject):JsonObject[] {
 const observations=object(client.observations),rows:JsonObject[]=[];
 for(const [key,value] of Object.entries(observations)){
  const row=object(value),values=object(row.values),facts=Array.isArray(values.facts)?values.facts:[];
  const counters=facts.flatMap(f=>Array.isArray(at(f,'fields'))?at(f,'fields') as JsonObject[]:[]).filter(f=>['Bonus Commissions Remaining','Remaining Charges'].includes(String(f.label))&&number(f.value)!==null);
  for(const counter of counters){const n=number(counter.value)!,current=row.currentStateVerified===true;
   rows.push({activityId:key+'.'+String(counter.label).toLowerCase().replaceAll(' ','_'),name:String(counter.label),status:current?'known':'stale',eligible:null,remainingCharges:n,completedToday:current?n===0:null,completedAtObservation:n===0,observationSource:'client_observation',observedAt:row.observedAt,scope:'member',nextReset:null,rewards:null,reasonCode:current?'explicit_current_displayed_remaining_counter':'older_observation_does_not_establish_today',evidence:{sourceKey:key,label:counter.label}});
  }
 }
 return rows;
}
