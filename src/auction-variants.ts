import { object, type JsonObject } from './types.js';
import { petProgress } from './progression.js';
export const SCROLLS = ['IMPLOSION_SCROLL','SHADOW_WARP_SCROLL','WITHER_SHIELD_SCROLL'] as const;
export const WITHER_BLADES = new Set(['HYPERION','SCYLLA','ASTRAEA','VALKYRIE','NECRON_BLADE']);
export function auctionVariants(id: string, extra: JsonObject): JsonObject {
  if (id!=='PET'&&!WITHER_BLADES.has(id)) return {};
  let scrolls: string[] | null = null;
  if (WITHER_BLADES.has(id)) {
    const value=extra.ability_scroll;
    scrolls=value===undefined?[]:Array.isArray(value)&&value.length<=3&&value.every(v=>typeof v==='string'&&SCROLLS.includes(v as typeof SCROLLS[number]))&&new Set(value).size===value.length?[...value].sort():null;
  }
  if (id!=='PET') return {scrolls,scrollCount:scrolls?.length??null,fullyScrolled:scrolls===null?null:scrolls.length===3};
  const pet=id==='PET'&&typeof extra.petInfo==='string'&&extra.petInfo.length<=4096?object(JSON.parse(extra.petInfo)):{};
  const progression=id==='PET'?petProgress(pet.type,pet.tier,pet.exp):null;
  const safeId=(v:unknown)=>typeof v==='string'&&/^[A-Z0-9_]{1,96}$/.test(v)?v:null;
  return {scrolls,scrollCount:scrolls?.length??null,fullyScrolled:scrolls===null?null:scrolls.length===3,petLevel:progression?.level??null,petMaxLevel:progression?.maxLevel??null,petExperience:id==='PET'&&typeof pet.exp==='number'&&Number.isFinite(pet.exp)&&pet.exp>=0?pet.exp:null,petRarity:safeId(pet.tier),petHeldItem:safeId(pet.heldItem),petSkin:safeId(pet.skin),petLevelReason:progression?.reasonCode??null};
}
