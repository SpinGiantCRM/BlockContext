import { parentPort } from 'node:worker_threads';
import { gunzipSync } from 'node:zlib';
import { parseUncompressed, simplify } from 'prismarine-nbt';
import { at, number, object } from './types.js';
import { auctionVariants } from './auction-variants.js';

// Batched parsing avoids one worker startup for every auction. No lore, sellers,
// bidder identities, timestamps embedded in NBT, or credentials leave this worker.
parentPort!.on('message',(values: unknown[]) => {
  let bytes = 0;
  const results = values.map(value => {
    try {
      if (typeof value !== 'string' || value.length > 128*1024 || value.length%4 || !/^[A-Za-z0-9+/]*={0,2}$/.test(value)) return null;
      bytes += value.length; if (bytes > 16*1024*1024) return null;
      const expanded = gunzipSync(Buffer.from(value,'base64'),{maxOutputLength:1024*1024});
      const items = at(simplify(parseUncompressed(expanded,'big')),'i');
      if (!Array.isArray(items) || items.length !== 1) return null;
      const item = object(items[0]), extra = object(at(item,'tag','ExtraAttributes'));
      if (typeof extra.id !== 'string' || !/^[A-Z0-9_:.-]{1,96}$/.test(extra.id) || !Number.isInteger(item.Count) || Number(item.Count) < 1 || Number(item.Count)>64) return null;
      const modifier = typeof extra.modifier === 'string' && /^[a-z0-9_]{1,64}$/.test(extra.modifier) ? extra.modifier : null;
      const stars = number(extra.upgrade_level) ?? number(extra.dungeon_item_level) ?? 0;
      if (!Number.isInteger(stars) || stars < 0 || stars > 15) return null;
      let petType: string | null = null;
      if (extra.id === 'PET') {
        if (typeof extra.petInfo !== 'string' || extra.petInfo.length>4096) return null;
        const pet = object(JSON.parse(extra.petInfo));
        if (typeof pet.type !== 'string' || !/^[A-Z0-9_]{1,64}$/.test(pet.type)) return null;
        petType = pet.type;
      }
      return { id: extra.id, count: item.Count, reforge: modifier, stars, petType, variants: auctionVariants(extra.id,extra) };
    } catch { return null; }
  });
  parentPort!.postMessage(results);
});
