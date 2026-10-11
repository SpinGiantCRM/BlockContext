import { parentPort, workerData } from 'node:worker_threads';
import { gunzipSync } from 'node:zlib';
import { parseUncompressed, simplify } from 'prismarine-nbt';
import { projectItems, type Item } from './inventory.js';

let slots = 0, expanded = 0;
const results: (Item[] | null)[] = [];
for (const data of workerData as unknown[]) {
  try {
    if (typeof data !== 'string' || data.length > 3*1024*1024 || data.length%4 || !/^[A-Za-z0-9+/]*={0,2}$/.test(data)) throw new Error('encoding');
    const compressed = Buffer.from(data,'base64');
    if (compressed.length > 2*1024*1024) throw new Error('compressed limit');
    const buffer = gunzipSync(compressed,{maxOutputLength: 8*1024*1024});
    expanded += buffer.length;
    if (expanded > 16*1024*1024) throw new Error('combined expansion limit');
    const value = simplify(parseUncompressed(buffer,'big')) as { i?: unknown[] };
    slots += Array.isArray(value.i) ? value.i.length : 4097;
    if (slots > 4096) throw new Error('combined slot limit');
    results.push(projectItems(value));
  } catch { results.push(null); }
}
parentPort!.postMessage(results);
