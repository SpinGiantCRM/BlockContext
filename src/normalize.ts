import { decodeInventory, type Item } from './inventory.js';
import { AppError, at, number, numericMap, object, text, type JsonObject, type NormalizedProfile, type Section } from './types.js';
import { skillSection } from './skills.js';
import { accessoryDiagnostics, describeItems, magicalPower } from './accessories.js';
import { gardenSection } from './garden.js';
import { uuidValue } from './upstream.js';
import { backpacks } from './backpacks.js';
import { greenhouse } from './greenhouse.js';

function section(data: JsonObject, present: boolean, warnings: string[] = []): Section {
  return { status: !present ? 'unavailable' : warnings.length ? 'partial' : 'available', data, warnings: !present ? ['Not available in the API response; this does not mean zero.', ...warnings] : warnings };
}

async function inventory(member: JsonObject, keys: string[][]): Promise<{ items: Item[] | null; warnings: string[] }> {
  for (const path of keys) {
    const value = at(member, 'inventory', ...path);
    if (at(value, 'data') === undefined) continue;
    try { return { items: await decodeInventory(value), warnings: [] }; }
    catch { return { items: null, warnings: ['Inventory data was present but could not be decoded safely.'] }; }
  }
  return { items: null, warnings: ['Inventory data not present; it may be disabled or unavailable.'] };
}

export async function normalizeProfile(raw: JsonObject, uuid: string, catalog: JsonObject, itemsCatalog: JsonObject = {}, playerMetadata: JsonObject = {}): Promise<NormalizedProfile> {
  const id = typeof raw.profile_id === 'string' ? uuidValue(raw.profile_id) : null;
  const memberValue = at(raw, 'members', uuid);
  if (!id || memberValue === undefined || !Object.keys(object(memberValue)).length) throw new AppError('UPSTREAM_INVALID', 'The requested profile or membership is invalid.');
  const m = object(memberValue);
  const statsXp = number(at(m, 'leveling', 'experience'));
  const stats = section({ skyBlockExperience: statsXp, skyBlockLevel: statsXp === null || statsXp < 0 ? null : Math.floor(statsXp / 100), experienceToNextSkyBlockLevel: statsXp === null || statsXp < 0 ? null : 100 - statsXp % 100, fairySoulsCollected: number(at(m, 'fairy_soul', 'total_collected')) }, statsXp !== null);
  const purse = number(at(m, 'currencies', 'coin_purse'));
  const bank = number(at(raw, 'banking', 'balance'));
  const currencies = section({ purseCoins: purse, sharedProfileBankCoins: bank, motes: number(at(m, 'currencies', 'motes_purse')) }, purse !== null || bank !== null);
  const bosses = object(at(m, 'slayer', 'slayer_bosses'));
  const slayers = section({ bosses: Object.fromEntries(Object.entries(bosses).slice(0, 32).map(([k, v]) => [k.slice(0, 64), { experience: number(at(v, 'xp')), killsByTier: numericMap(Object.fromEntries(Object.entries(object(v)).filter(([key]) => key.startsWith('boss_kills_tier_'))), 64) }])) }, at(m, 'slayer', 'slayer_bosses') !== undefined);
  const dungeonTypes = object(at(m, 'dungeons', 'dungeon_types'));
  const dungeons = section({ types: Object.fromEntries(Object.entries(dungeonTypes).slice(0, 16).map(([k, v]) => [k.slice(0, 64), { experience: number(at(v, 'experience')), highestTierCompleted: number(at(v, 'highest_tier_completed')), completions: numericMap(at(v, 'tier_completions'), 32) }])), classes: Object.fromEntries(Object.entries(object(at(m, 'dungeons', 'player_classes'))).slice(0, 16).map(([k, v]) => [k.slice(0, 64), { experience: number(at(v, 'experience')) }])) }, at(m, 'dungeons', 'dungeon_types') !== undefined);
  const rawPets = at(m, 'pets_data', 'pets');
  const pets = section({ pets: Array.isArray(rawPets) ? rawPets.slice(0, 512).map(v => ({ type: text(at(v, 'type')), tier: text(at(v, 'tier')), experience: number(at(v, 'exp')), active: typeof at(v, 'active') === 'boolean' ? at(v, 'active') : null, heldItem: text(at(v, 'heldItem')) })) : null }, Array.isArray(rawPets));
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
  const wardrobe = await inventory(m, [['wardrobe_contents']]);
  const backpack = await backpacks(at(m,'inventory','backpack_contents'));
  const sacksRaw = at(m, 'inventory', 'sacks_counts');
  const sacksPresent = sacksRaw !== null && typeof sacksRaw === 'object' && !Array.isArray(sacksRaw);
  const sacks = sacksPresent ? numericMap(sacksRaw) : null;
  const storageCoverage = Object.fromEntries([['enderChestItems', enderChest], ['personalVaultItems', vault], ['wardrobeItems', wardrobe]].map(([name,value]) => [name, (value as typeof vault).items === null ? 'unavailable' : 'available']));
  storageCoverage.sackCounts = sacksPresent ? 'available' : 'unavailable';
  storageCoverage.backpackItems = backpack.status;
  const equipmentSection = section({
    armor: describeItems(armor.items, itemsCatalog), equipment: describeItems(equipment.items, itemsCatalog), carriedItems: describeItems(carried.items, itemsCatalog),
    enderChestItems: describeItems(enderChest.items, itemsCatalog), personalVaultItems: describeItems(vault.items, itemsCatalog), wardrobeItems: describeItems(wardrobe.items, itemsCatalog), sackCounts: sacks,
    backpackItems: backpack.items === null ? null : backpack.items.map(item => ({ ...describeItems([item],itemsCatalog)![0], containerId: item.containerId })), backpackContainers: backpack.containers,
    backpackItemCount: backpack.status === 'available' ? backpack.items!.length : null, knownBackpackItemCount: backpack.items?.length ?? null,
    storageCoverage, storageWarnings: [...enderChest.warnings.map(w => `Ender Chest: ${w}`), ...vault.warnings.map(w => `Personal vault: ${w}`), ...wardrobe.warnings.map(w => `Wardrobe: ${w}`), ...backpack.warnings],
    itemCatalogLastUpdated: number(itemsCatalog.lastUpdated), itemCatalogAvailable: Object.keys(object(itemsCatalog.items)).length > 0
  }, armor.items !== null || equipment.items !== null || carried.items !== null, [...armor.warnings.map(w => `Armor: ${w}`), ...equipment.warnings.map(w => `Equipment: ${w}`), ...carried.warnings.map(w => `Carried inventory: ${w}`)]);
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
