import { decodeInventory, type Item } from './inventory.js';
import { AppError, at, number, numericMap, object, text, type JsonObject, type NormalizedProfile, type Section } from './types.js';
import { skillSection } from './skills.js';
import { accessoryDiagnostics, describeItems, magicalPower } from './accessories.js';
import { gardenSection } from './garden.js';
import { uuidValue } from './upstream.js';
import { backpacks } from './backpacks.js';
import { loadouts } from './loadouts.js';
import { petsSection, dungeonsSection } from './progression.js';
import { greenhouse } from './greenhouse.js';
import { buffs } from './buffs.js';

function section(data: JsonObject, present: boolean, warnings: string[] = []): Section {
  return { status: !present ? 'unavailable' : warnings.length ? 'partial' : 'available', data, warnings: !present ? ['Not available in the API response; this does not mean zero.', ...warnings] : warnings };
}

async function inventory(member: JsonObject, keys: string[][]): Promise<{ items: Item[] | null; warnings: string[]; reasonCode: string | null }> {
  for (const path of keys) {
    const value = at(member, 'inventory', ...path);
    if (value===undefined) continue;
    if (at(value, 'data')===undefined) return {items:null,reasonCode:'invalid_api_value',warnings:['Inventory field is present but its encoding is missing or invalid.']};
    try { return { items: await decodeInventory(value), warnings: [], reasonCode: null }; }
    catch { return { items: null, reasonCode: 'decode_or_safety_limit', warnings: ['Inventory data was present but could not be decoded safely.'] }; }
  }
  return { items: null, reasonCode: 'not_exposed', warnings: ['Inventory data not present; it may be disabled or unavailable.'] };
}

export async function normalizeProfile(raw: JsonObject, uuid: string, catalog: JsonObject, itemsCatalog: JsonObject = {}, playerMetadata: JsonObject = {}): Promise<NormalizedProfile> {
  const id = typeof raw.profile_id === 'string' ? uuidValue(raw.profile_id) : null;
  const memberValue = at(raw, 'members', uuid);
  if (!id || memberValue === undefined || !Object.keys(object(memberValue)).length) throw new AppError('UPSTREAM_INVALID', 'The requested profile or membership is invalid.');
  const m = object(memberValue);
  const statsXp = number(at(m, 'leveling', 'experience'));
  const fairy=number(at(m,'fairy_soul','total_collected'));
  const stats = section({ skyBlockExperience: statsXp!==null&&statsXp>=0?statsXp:null, skyBlockLevel: statsXp === null || statsXp < 0 ? null : Math.floor(statsXp / 100), experienceToNextSkyBlockLevel: statsXp === null || statsXp < 0 ? null : 100 - statsXp % 100, fairySoulsCollected: fairy!==null&&fairy>=0?fairy:null,
    fieldCoverage:{level:{status:statsXp!==null&&statsXp>=0?'available':'unavailable',reasonCode:statsXp!==null&&statsXp>=0?null:'not_exposed_or_invalid'},fairySouls:{status:fairy!==null&&fairy>=0?'available':'unavailable',reasonCode:fairy!==null&&fairy>=0?null:'not_exposed_or_invalid'}} }, statsXp!==null&&statsXp>=0||fairy!==null&&fairy>=0, statsXp===null||statsXp<0||fairy===null||fairy<0?['Some SkyBlock level or fairy soul fields are unavailable or invalid.']:[]);
  const buff=buffs(m), statsPresent=stats.status!=='unavailable';
  stats.data={...stats.data,...buff.data,fieldCoverage:{...object(stats.data.fieldCoverage),...buff.fieldCoverage}};
  stats.status=!statsPresent&&!buff.anyAvailable?'unavailable':stats.status!=='available'||buff.incomplete?'partial':'available';
  stats.warnings.push(...buff.warnings);
  const purse = number(at(m, 'currencies', 'coin_purse'));
  const bank = number(at(raw, 'banking', 'balance'));
  const personalBank = number(at(m,'profile','bank_account')), motes=number(at(m,'currencies','motes_purse'));
  const moneyFields={purseCoins:purse,sharedProfileBankCoins:bank,personalBankCoins:personalBank,motes};
  const moneyRaw={purseCoins:at(m,'currencies','coin_purse'),sharedProfileBankCoins:at(raw,'banking','balance'),personalBankCoins:at(m,'profile','bank_account'),motes:at(m,'currencies','motes_purse')};
  const moneySources={purseCoins:'member.currencies.coin_purse',sharedProfileBankCoins:'profile.banking.balance',personalBankCoins:'member.profile.bank_account',motes:'member.currencies.motes_purse'};
  const moneyCoverage=Object.fromEntries(Object.entries(moneyFields).map(([key,value])=>[key,{status:value!==null&&value>=0?'available':'unavailable',reasonCode:moneyRaw[key as keyof typeof moneyRaw]===undefined?'not_exposed':value===null||value<0?'invalid_api_value':null,source:moneySources[key as keyof typeof moneySources]}]));
  const currencies = section({ ...Object.fromEntries(Object.entries(moneyFields).map(([k,v])=>[k,v!==null&&v>=0?v:null])), fieldCoverage:moneyCoverage }, Object.values(moneyFields).some(v=>v!==null&&v>=0), Object.values(moneyCoverage).some(v=>v.status!=='available')?['Some balances are not exposed or invalid; they remain unknown. Personal and shared profile bank balances are separate. Missing API data alone does not establish whether a privacy setting is disabled.']:[]);
  const bosses = object(at(m, 'slayer', 'slayer_bosses'));
  const slayers = section({ bosses: Object.fromEntries(Object.entries(bosses).slice(0, 32).map(([k, v]) => [k.slice(0, 64), { experience: number(at(v, 'xp')), killsByTier: numericMap(Object.fromEntries(Object.entries(object(v)).filter(([key]) => key.startsWith('boss_kills_tier_'))), 64) }])) }, at(m, 'slayer', 'slayer_bosses') !== undefined);
  const dungeons = dungeonsSection(m);
  const pets = petsSection(at(m,'pets_data','pets'));
  const collections = section({ collected: numericMap(m.collection) }, m.collection !== undefined);
  const generators = at(m, 'player_data', 'crafted_generators') ?? m.crafted_generators;
  const minions = section({ craftedGenerators: Array.isArray(generators) ? generators.slice(0, 2048).map(v => text(v)).filter(Boolean) : null, placedMinions: null }, Array.isArray(generators), ['Crafted generators are unlock history, not the currently placed minion setup.']);
  // Limit simultaneous decoding and isolate parser work from the request thread.
  const armor = await inventory(m, [['inv_armor']]);
  const equipment = await inventory(m, [['equipment_contents']]);
  const carried = await inventory(m, [['inv_contents']]);
  const accessories = await inventory(m, [['bag_contents', 'talisman_bag'], ['talisman_bag']]);
  const enderChest = await inventory(m, [['ender_chest_contents']]);
  const vault = await inventory(m, [['personal_vault_contents']]);
  const invalidLoadout=m.loadout!==undefined&&(m.loadout===null||typeof m.loadout!=='object'||Array.isArray(m.loadout));
  const modernArmor = await loadouts(invalidLoadout?null:at(m,'loadout','armor'),'armor');
  const modernEquipment = await loadouts(invalidLoadout?null:at(m,'loadout','equipment'),'equipment');
  const legacyWardrobe = !invalidLoadout&&at(m,'loadout','armor')===undefined ? await inventory(m, [['wardrobe_contents']]) : null;
  const wardrobe = legacyWardrobe ?? modernArmor;
  const backpack = await backpacks(at(m,'inventory','backpack_contents'));
  const sacksRaw = at(m, 'inventory', 'sacks_counts');
  const sacksPresent = sacksRaw !== null && typeof sacksRaw === 'object' && !Array.isArray(sacksRaw);
  const sacks = sacksPresent ? numericMap(sacksRaw) : null;
  const storageCoverage = Object.fromEntries([['enderChestItems', enderChest], ['personalVaultItems', vault], ['wardrobeItems', wardrobe]].map(([name,value]) => [name, (value as typeof vault).items === null ? 'unavailable' : 'available']));
  storageCoverage.equipmentWardrobeItems = modernEquipment.status;
  storageCoverage.wardrobeItems = legacyWardrobe ? (legacyWardrobe.items===null?'unavailable':'available') : modernArmor.status;
  storageCoverage.sackCounts = sacksPresent ? 'available' : 'unavailable';
  storageCoverage.backpackItems = backpack.status;
  const equipmentSection = section({
    fieldCoverage: Object.fromEntries([['armor',armor],['equipment',equipment],['carriedItems',carried],['enderChestItems',enderChest],['personalVaultItems',vault],['wardrobeItems',wardrobe],['equipmentWardrobeItems',modernEquipment]].map(([name,value])=>{const v=value as typeof wardrobe;return [name,{status:'status' in v?v.status:v.items===null?'unavailable':'available',reasonCode:v.reasonCode,source:name==='wardrobeItems'?(legacyWardrobe?'inventory.wardrobe_contents':'loadout.armor'):name==='equipmentWardrobeItems'?'loadout.equipment':'inventory'}];}).concat([
      ['backpackItems',{status:backpack.status,reasonCode:backpack.status==='available'?null:at(m,'inventory','backpack_contents')===undefined?'not_exposed':'decode_or_invalid_api_value',source:'inventory.backpack_contents'}],
      ['sackCounts',{status:sacksPresent?'available':'unavailable',reasonCode:sacksPresent?null:'not_exposed_or_invalid',source:'inventory.sacks_counts'}]
    ])),
    wardrobeSets: legacyWardrobe ? null : modernArmor.sets, equipmentWardrobeSets: modernEquipment.sets,
    equipmentWardrobeItems: describeItems(modernEquipment.items,itemsCatalog),
    wardrobeItemCount: storageCoverage.wardrobeItems==='available'?wardrobe.items!.length:null, knownWardrobeItemCount:wardrobe.items?.length??null,
    armor: describeItems(armor.items, itemsCatalog), equipment: describeItems(equipment.items, itemsCatalog), carriedItems: describeItems(carried.items, itemsCatalog),
    enderChestItems: describeItems(enderChest.items, itemsCatalog), personalVaultItems: describeItems(vault.items, itemsCatalog), wardrobeItems: describeItems(wardrobe.items, itemsCatalog), sackCounts: sacks,
    backpackItems: backpack.items === null ? null : backpack.items.map(item => ({ ...describeItems([item],itemsCatalog)![0], containerId: item.containerId })), backpackContainers: backpack.containers,
    backpackItemCount: backpack.status === 'available' ? backpack.items!.length : null, knownBackpackItemCount: backpack.items?.length ?? null,
    storageCoverage, storageWarnings: [...enderChest.warnings.map(w => `Ender Chest: ${w}`), ...vault.warnings.map(w => `Personal vault: ${w}`), ...wardrobe.warnings.map(w => `Wardrobe: ${w}`), ...modernEquipment.warnings, ...backpack.warnings],
    itemCatalogLastUpdated: number(itemsCatalog.lastUpdated), itemCatalogAvailable: Object.keys(object(itemsCatalog.items)).length > 0
  }, armor.items !== null || equipment.items !== null || carried.items !== null || wardrobe.items !== null || modernEquipment.items !== null || backpack.items !== null, [...armor.warnings.map(w => `Armor: ${w}`), ...equipment.warnings.map(w => `Equipment: ${w}`), ...carried.warnings.map(w => `Carried inventory: ${w}`), ...(Object.values(storageCoverage).some(v=>v!=='available')?['Some storage fields are unavailable or partial; inspect fieldCoverage and storageWarnings.']:[])]);
  const storage = object(m.accessory_bag_storage);
  const bagItems = describeItems(accessories.items, itemsCatalog);
  const diagnostics = accessoryDiagnostics(bagItems, describeItems(carried.items, itemsCatalog));
  const power = magicalPower(bagItems, m);
  const accessorySection = section({
    bagItems, ...diagnostics, selectedPower: text(storage.selected_power),
    unlockedPowers: Array.isArray(storage.unlocked_powers) ? storage.unlocked_powers.slice(0, 256).map(v => text(v)).filter(Boolean) : null,
    tuning: at(storage, 'tuning', 'slot_0') === undefined ? null : numericMap(at(storage, 'tuning', 'slot_0'), 32),
    highestRecordedMagicalPower: number(storage.highest_magical_power), ...power,
    itemCatalogLastUpdated: number(itemsCatalog.lastUpdated), itemCatalogAvailable: Object.keys(object(itemsCatalog.items)).length > 0,
    unclassifiedCarriedItems: carried.items === null ? null : describeItems(carried.items, itemsCatalog)!.filter(i => i.category === null).length
  }, accessories.items !== null || Object.keys(storage).length > 0,
    [...accessories.warnings, 'Highest recorded magical power is historical and may differ from the current snapshot calculation. The calculation applies outside Dungeons and uses pinned community rules.',
    ...(power.reasons as string[]), 'Carried accessory lists include only items identified by the catalog or a structured footer. Stored or carried items do not add bag magical power.']);
  const garden = gardenSection(null); garden.data.greenhouse = greenhouse(m);
  return { id, name: text(raw.cute_name, 32) ?? 'Unnamed', selected: raw.selected === true, gameMode: text(raw.game_mode, 32), sections: { stats, skills: skillSection(m, catalog, playerMetadata), currencies, slayers, dungeons, pets, collections, minions, equipment: equipmentSection, accessories: accessorySection, garden } };
}
