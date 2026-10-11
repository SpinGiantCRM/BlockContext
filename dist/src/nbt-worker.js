import { parentPort, workerData } from 'node:worker_threads';
import { gunzipSync } from 'node:zlib';
import { parseUncompressed, simplify } from 'prismarine-nbt';
import { projectItems } from './inventory.js';
try {
    const compressed = Buffer.from(workerData, 'base64');
    if (compressed.length > 2 * 1024 * 1024)
        throw new Error('compressed limit');
    const buffer = gunzipSync(compressed, { maxOutputLength: 8 * 1024 * 1024 });
    const parsed = parseUncompressed(buffer, 'big');
    parentPort.postMessage({ items: projectItems(simplify(parsed)) });
}
catch {
    parentPort.postMessage({ error: 'invalid inventory' });
}
