import { readFileSync } from 'node:fs';
import { at, number, object, type JsonObject, type Section } from './types.js';

interface Rules {
  sourceCommit: string; petSource: string; dungeonSource: string;
  petRarityOffsets: Record<string, number>; petXpSteps: number[];
  customPetLeveling: Record<string, {type?: number; pet_levels?: number[]; max_level?: number; rarity_offset?: Record<string,number>}>;
  knownPetTypes: string[]; catacombsXpSteps: number[]; catacombsCap: number;
  normalFloorLevelRequirements: number[]; floorRequirementSource: string; floorRequirementReview: { checkedAt: string; status: string; sources: string[] };
}
const rules: Rules = JSON.parse(readFileSync(new URL('../../fixtures/domain/progression.json', import.meta.url),'utf8'));
const petTypes = new Set(rules.knownPetTypes);
export const PROGRESSION_REVISION = rules.sourceCommit;

function progress(xp: number, steps: number[], start: number): JsonObject {
  let level = start, base = 0, next: number | null = null;
  for (const step of steps) {
    if (xp >= base + step) { base += step; level++; }
    else { next = base + step; break; }
  }
  const maximumExperience = steps.reduce((n,v)=>n+v,0), maxLevel = start+steps.length;
  return { level, levelWithProgress: level+(next===null?0:(xp-base)/(next-base)), maxLevel, atCap:level===maxLevel,
    experienceIntoCurrentLevel:next===null?0:xp-base, experienceToNextLevel:next===null?null:next-xp,
    progressToNextLevel:next===null?null:(xp-base)/(next-base), maximumExperience,
    experienceToMaxLevel:Math.max(0,maximumExperience-xp), progressToMaxLevel:Math.min(1,xp/maximumExperience), overflowExperience:Math.max(0,xp-maximumExperience) };
}
export function petProgress(type: unknown, tier: unknown, experience: unknown): JsonObject {
  const unknown = (reasonCode: string): JsonObject => ({status:'unavailable',reasonCode,level:null,maxLevel:null,experienceToNextLevel:null,experienceToMaxLevel:null,progressToMaxLevel:null,sourceRevision:PROGRESSION_REVISION});
  const xp = number(experience);
  if (xp===null || xp<0) return unknown('missing_or_invalid_experience');
  if (typeof type!=='string' || !petTypes.has(type)) return unknown('unsupported_pet_type');
  if (typeof tier!=='string') return unknown('unsupported_pet_rarity');
  const custom=rules.customPetLeveling[type], offset=(custom?.rarity_offset??rules.petRarityOffsets)[tier];
  if (offset===undefined || !Number.isInteger(offset)) return unknown('unsupported_pet_rarity');
  const table=custom?.type===1?[...rules.petXpSteps,...custom.pet_levels??[]]:custom?.type===2?custom.pet_levels??[]:rules.petXpSteps;
  const cap=custom?.max_level??100, steps=table.slice(offset,offset+cap-1);
  if (steps.length!==cap-1 || steps.some(v=>!Number.isFinite(v)||v<=0)) return unknown('unsupported_experience_curve');
  // XP gain multipliers are already reflected in API exp; never apply them again.
  const value=progress(xp,steps,1);
  return {status:'available',reasonCode:null,...value,dragonStage:cap===200?(Number(value.level)<100?'egg':'hatched'):null,sourceRevision:PROGRESSION_REVISION};
}
export function petsSection(raw: unknown): Section {
  if (!Array.isArray(raw)) return {status:'unavailable',data:{pets:null,fieldCoverage:{pets:{status:'unavailable',reasonCode:raw===undefined?'not_exposed':'invalid_api_value'}}},warnings:['Pet list is not exposed as a valid API array; this does not mean no pets.']};
  const pets=raw.slice(0,512).map(v=>{
    const p=object(v), calculation=petProgress(p.type,p.tier,p.exp);
    return {type:typeof p.type==='string'&&/^[A-Z0-9_]{1,64}$/.test(p.type)?p.type:null,tier:typeof p.tier==='string'&&/^[A-Z_]{1,16}$/.test(p.tier)?p.tier:null,experience:number(p.exp),active:typeof p.active==='boolean'?p.active:null,heldItem:typeof p.heldItem==='string'&&/^[A-Z0-9_]{1,96}$/.test(p.heldItem)?p.heldItem:null,level:calculation.level,maxLevel:calculation.maxLevel,progression:calculation};
  });
  const unknownLevels=pets.filter(p=>p.progression.status!=='available').length, unknownActive=pets.filter(p=>p.active===null).length, truncated=raw.length>512;
  return {status:unknownLevels||unknownActive||truncated?'partial':'available',data:{pets,fieldCoverage:{pets:{status:truncated?'partial':'available',reasonCode:truncated?'safety_limit':null},levels:{status:unknownLevels?'partial':'available',unknownCount:unknownLevels,reasonCode:unknownLevels?'unsupported_or_missing_pet_data':null},activeFlags:{status:unknownActive?'partial':'available',unknownCount:unknownActive}}},warnings:['Pet levels use pinned community XP tables; unknown types and rarities remain unknown.',...(unknownLevels?['Some pets cannot be converted to levels; inspect each progression reasonCode.']:[]),...(truncated?['Pet list exceeds the 512-entry safety limit.']:[])]};
}
export function dungeonProgress(experience: unknown): JsonObject {
  const xp=number(experience);
  if (xp===null||xp<0) return {status:'unavailable',reasonCode:'missing_or_invalid_experience',effectiveLevel:null,experienceToNextLevel:null,sourceRevision:PROGRESSION_REVISION};
  const value=progress(xp,rules.catacombsXpSteps,0);
  return {status:'available',reasonCode:null,...value,effectiveLevel:value.level,effectiveLevelWithProgress:value.levelWithProgress,sourceRevision:PROGRESSION_REVISION};
}
export function dungeonsSection(member: JsonObject): Section {
  const raw=at(member,'dungeons','dungeon_types'), types: JsonObject={}, classes:JsonObject={}, fieldCoverage:JsonObject={};
  for (const [k,v] of Object.entries(object(raw)).slice(0,16)) {
    if (!/^[a-z_]{1,32}$/.test(k)) continue;
    const xp=number(at(v,'experience')), calculation=k==='catacombs'?dungeonProgress(xp):null, completions=at(v,'tier_completions');
    const completionMap=completions!==null&&typeof completions==='object'&&!Array.isArray(completions);
    const validCompletions=completionMap&&Object.entries(object(completions)).length<=32&&Object.entries(object(completions)).every(([floor,count])=>/^[0-7]$/.test(floor)&&Number.isInteger(count)&&Number(count)>=0);
    const rawHighest=number(at(v,'highest_tier_completed')), highest=rawHighest!==null&&Number.isInteger(rawHighest)&&rawHighest>=0&&rawHighest<=7?rawHighest:null;
    const nextFloor=calculation?.status==='available'?rules.normalFloorLevelRequirements.findIndex(l=>l>Number(calculation.effectiveLevel)):-1;
    types[k]={experience:xp,highestTierCompleted:highest,completions:validCompletions?Object.fromEntries(Object.entries(object(completions)).filter(([floor,count])=>/^[0-7]$/.test(floor)&&number(count)!==null&&Number(count)>=0)):null,
      progression:calculation,effectiveLevel:calculation?.effectiveLevel??null,
      nextUnlock:nextFloor<0?null:{kind:'normal_floor_level_gate',floor:nextFloor+1,requiredCatacombsLevel:rules.normalFloorLevelRequirements[nextFloor],levelsRemaining:rules.normalFloorLevelRequirements[nextFloor]!-Number(calculation!.effectiveLevel),otherPrerequisitesAssessed:false,referenceSource:rules.floorRequirementSource,referenceDate:'2021-03-30',referenceReview:rules.floorRequirementReview}};
    fieldCoverage[k]={experience:{status:xp!==null&&xp>=0?'available':'unavailable',reasonCode:xp!==null&&xp>=0?null:'missing_or_invalid_experience'},highestTierCompleted:{status:highest!==null&&Number.isInteger(highest)&&highest>=0&&highest<=7?'available':'unavailable',reasonCode:highest===null?'not_exposed_or_invalid':null},completions:{status:validCompletions?'available':'unavailable',reasonCode:validCompletions?null:'not_exposed_or_invalid'},level:{status:calculation?.status??'unavailable',reasonCode:calculation?calculation.reasonCode:'unsupported_dungeon_type'}};
  }
  for (const [k,v] of Object.entries(object(at(member,'dungeons','player_classes'))).slice(0,16)) {
    if (!/^[a-z_]{1,32}$/.test(k)) continue;
    classes[k]={experience:number(at(v,'experience')),progression:dungeonProgress(at(v,'experience'))};
  }
  const present=raw!==undefined&&raw!==null&&typeof raw==='object'&&!Array.isArray(raw);
  const incomplete=!Object.keys(types).length||Object.values(types).some(v=>at(v,'progression','status')!=='available'||at(v,'completions')===null||at(v,'highestTierCompleted')===null)||at(member,'dungeons','player_classes')===undefined||Object.values(classes).some(v=>at(v,'progression','status')!=='available');
  return {status:!present?'unavailable':incomplete?'partial':'available',data:{types,classes,fieldCoverage,levelCap:rules.catacombsCap,floorRequirementReview:rules.floorRequirementReview},warnings:['Levels use pinned community XP tables and stop at level 50; overflow XP is separate. Next unlock is the next normal-floor level gate from the cited patch, not permission to enter: Combat level, prior floor completions and any later rule changes must also be checked. Current floor requirements remain unverified; referenceReview records the latest source review.',...(incomplete?['Some dungeon XP, class data or floor completions are missing or unsupported.']:[])]};
}
