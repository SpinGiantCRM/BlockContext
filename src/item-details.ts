import {mapGameFields} from './factual-fields.js';
import {createHash} from 'node:crypto';
import {at,number,numericMap,object,text,type JsonObject} from './types.js';

export function storedWeaponFromLore(lore:unknown):JsonObject|null {
  if(!Array.isArray(lore))return null;
  const reports=lore.map(l=>text(l,160)).filter((l):l is string=>typeof l==='string'&&l.startsWith('Stored Weapon: '));
  if(reports.length!==1)return null;
  const match=reports[0]!.match(/^Stored Weapon: ([A-Za-z][A-Za-z '-]{0,95}?)(?: ([✪★]{1,15}))?$/);
  if(!match)return null;
  if(['None','No Weapon'].includes(match[1]!))return {status:'known_empty',displayName:null,stars:null,completeWeaponDetails:false};
  return {status:'partial',displayName:match[1]!.trim(),stars:match[2]?.length??null,id:null,reforge:null,enchantments:null,completeWeaponDetails:false,reasonCode:'tooltip_identity_and_displayed_stars_only'};
}
export function itemDetails(extra:JsonObject,lore:unknown):JsonObject {
  const out:JsonObject={};
  if(typeof extra.uuid==='string'&&/^[a-f0-9-]{32,36}$/i.test(extra.uuid))out.instanceId=createHash('sha256').update(extra.uuid.toLowerCase().replaceAll('-','')).digest('hex');
  const upgrades:JsonObject={};
  for(const key of ['hot_potato_count','rarity_upgrades','upgrade_level','dungeon_item_level','art_of_war_count','art_of_peace_count','farming_for_dummies_count','wood_singularity_count','tuned_transmission','mana_disintegrator_count','etherwarp','drill_fuel','drill_fuel_capacity','fuel','pickonimbus_durability','champion_combat_xp','cultivating','farmed_cultivating','compact_blocks','expertise_kills','bass_weight','edition']){
    const n=number(extra[key]);if(n!==null&&n>=0&&n<=1e15)upgrades[key]=n;
  }
  out.upgrades=upgrades;
  out.abilityScrolls=Array.isArray(extra.ability_scroll)?extra.ability_scroll.filter(v=>typeof v==='string'&&/^[A-Z_]{1,96}$/.test(v)).slice(0,16):null;
  out.attributes=extra.attributes===undefined?null:numericMap(extra.attributes,64);
  out.runes=extra.runes===undefined?null:numericMap(extra.runes,32);
  const gems=object(extra.gems),safeGems:JsonObject={};
  for(const [k,v] of Object.entries(gems).slice(0,64)){
    if(!/^[A-Z0-9_]{1,64}$/.test(k))continue;
    if(typeof v==='string'&&/^[A-Z0-9_]{1,32}$/.test(v))safeGems[k]=v;
    else if(v!==null&&typeof v==='object'&&!Array.isArray(v))safeGems[k]=Object.fromEntries(Object.entries(object(v)).filter(([key,x])=>['quality','type'].includes(key)&&typeof x==='string'&&/^[A-Z0-9_]{1,32}$/.test(x)));
  }
  out.gems=extra.gems===undefined?null:safeGems;
  for(const key of ['drill_part_engine','drill_part_fuel_tank','drill_part_upgrade_module','power_ability_scroll','skin'])if(typeof extra[key]==='string'&&/^[A-Za-z0-9_:.-]{1,96}$/.test(extra[key]))out[key]=extra[key];
  out.soulbound=typeof extra.donated_museum==='boolean'?extra.donated_museum:typeof extra.soulbound==='boolean'?extra.soulbound:null;
  out.storedWeapon=storedWeaponFromLore(lore);
  const attrs={...extra};if(typeof attrs.petInfo==='string')try{attrs.petInfo=JSON.parse(attrs.petInfo);}catch{attrs.petInfo=null;}
  const mapped=mapGameFields(attrs,'attributes','item',256);out.structuredAttributes=mapped;
  out.receivedAttributeKeys=Object.keys(attrs).filter(k=>!/(uuid|owner|donator|originator|history|timestamp|secret|token|email)/i.test(k)).slice(0,128);
  out.displayedStats=Array.isArray(lore)?lore.flatMap(raw=>{const line=text(raw,256),match=line?.match(/^([A-Za-z ]{1,40}): ([+-]?[\d,]+(?:\.\d+)?)(%?)(?: |$)/);return match?[{label:match[1],value:Number(match[2]!.replaceAll(',','')),unit:match[3]?'%':'stat',valueKind:'primary_displayed_item_stat'}]:[];}):null;
  out.detailCoverage={complete:false,rawLoreExposed:false,unknownAbsentUpgrades:true};
  return out;
}
export function huntaxeSummary(items:JsonObject[]|null):JsonObject {
  const tools=(items??[]).filter(i=>['VENATOR_GENESIS','SILVA_DOMINUS','CURSUS_FERAE','APEX_PRAEDATOR','NEX_TITANUM'].includes(String(i.id))||i.storedWeapon!==undefined&&i.storedWeapon!==null);
  const reports=tools.filter(i=>i.storedWeapon!==null&&i.storedWeapon!==undefined);
  if(!reports.length)return {status:'unavailable',reasonCode:'stored_weapon_not_exposed_in_observed_item_lore',weapon:null,source:'Hypixel profile inventory item lore'};
  const signatures=new Set(reports.map(i=>JSON.stringify(i.storedWeapon)));
  return {status:'partial',reasonCode:signatures.size>1?'conflicting_tooltip_reports':'tooltip_identity_and_displayed_stars_only',weapon:signatures.size===1?reports[0]!.storedWeapon:null,source:'Hypixel profile inventory item lore',tools:reports.map(i=>({id:i.id,instanceId:i.instanceId??null,slot:i.slot,location:i.location??null,storedWeapon:i.storedWeapon})),fullEnchantmentAndUpgradeDetailsKnown:false};
}
