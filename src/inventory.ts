import { Worker } from 'node:worker_threads';
import { AppError, at, number, numericMap, object, text, type JsonObject } from './types.js';
import {itemDetails} from './item-details.js';

export interface Item { slot: number; id: string | null; name: string | null; count: number | null; reforge: string | null; enchantments: Record<string, number>; rarityUpgrade: number | null; dungeonUpgradeLevel: number | null; rarity: string | null; loreCategory: string | null }
export function projectItems(value: unknown): Item[] {
  const entries = at(value, 'i');
  if (!Array.isArray(entries) || entries.length > 4096) throw new AppError('INVENTORY_INVALID', 'The decoded inventory has an invalid slot list.');
  return entries.flatMap((v, slot) => {
    const entry = object(v), extra = object(at(entry, 'tag', 'ExtraAttributes'));
    if (Object.keys(entry).length === 0 || number(entry.Count) === 0) return [];
    // Extract only the structured footer; arbitrary lore never leaves the worker.
    const lore = at(entry, 'tag', 'display', 'Lore');
    const footer = Array.isArray(lore) ? text(lore.at(-1), 96) : null;
    const parsed = footer?.match(/^(?:a )?(COMMON|UNCOMMON|RARE|EPIC|LEGENDARY|MYTHIC|DIVINE|SPECIAL|VERY SPECIAL) (?:DUNGEON )?([A-Z ]{1,32})(?: a)?$/);
    return [{ slot, id: text(extra.id), name: text(at(entry, 'tag', 'display', 'Name')), count: number(entry.Count), reforge: text(extra.modifier), enchantments: numericMap(extra.enchantments, 128), rarityUpgrade: number(extra.rarity_upgrades), dungeonUpgradeLevel: number(extra.upgrade_level) ?? number(extra.dungeon_item_level), rarity: parsed?.[1] ?? null, loreCategory: parsed?.[2]?.trim() ?? null,...itemDetails(extra,lore) }];
  });
}
export async function decodeInventory(value: unknown): Promise<Item[] | null> {
  const data = at(value, 'data');
  if (data === undefined) return null;
  if (typeof data !== 'string' || data.length > 3 * 1024 * 1024 || data.length % 4 !== 0 || !/^[A-Za-z0-9+/]*={0,2}$/.test(data))
    throw new AppError('INVENTORY_INVALID', 'The inventory encoding is invalid or too large.');
  return new Promise((resolve, reject) => {
    const worker = new Worker(new URL('./nbt-worker.js', import.meta.url), { workerData: data, execArgv: [], env: {}, resourceLimits: { maxOldGenerationSizeMb: 64, stackSizeMb: 2 } });
    // Include worker/module startup on small VPS hosts while keeping parsing bounded.
    const timeout = setTimeout(() => { void worker.terminate(); reject(new AppError('INVENTORY_INVALID', 'Inventory decoding exceeded its time limit.')); }, 5000);
    const finish = () => { clearTimeout(timeout); void worker.terminate(); };
    worker.once('message', (message: { items?: Item[]; error?: string }) => { finish(); if (message.items) resolve(message.items); else reject(new AppError('INVENTORY_INVALID', 'The inventory could not be decoded safely.')); });
    worker.once('error', () => { finish(); reject(new AppError('INVENTORY_INVALID', 'The inventory decoder failed.')); });
    worker.once('exit', code => { clearTimeout(timeout); if (code !== 0) reject(new AppError('INVENTORY_INVALID', 'The inventory decoder stopped.')); });
  });
}
