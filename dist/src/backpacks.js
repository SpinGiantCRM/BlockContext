import { Worker } from 'node:worker_threads';
import { at, object } from './types.js';
export async function backpacks(value) {
    const missing = { status: 'unavailable', items: null, containers: null, warnings: ['Backpack contents are not available. Icons alone do not establish contents or an empty backpack.'] };
    if (value === undefined || value === null)
        return missing;
    if (typeof value !== 'object' || Array.isArray(value))
        return { ...missing, warnings: ['Backpack contents have an invalid container map.'] };
    const entries = Object.entries(object(value));
    if (entries.length > 36 || entries.some(([id]) => !/^(0|[1-9][0-9]{0,2})$/.test(id)))
        return { ...missing, warnings: ['Backpack container identifiers or count exceed supported bounds.'] };
    entries.sort(([a], [b]) => Number(a) - Number(b));
    if (!entries.length)
        return { status: 'available', items: [], containers: [], warnings: [] };
    const encoded = entries.map(([, v]) => at(v, 'data'));
    if (encoded.reduce((n, v) => n + (typeof v === 'string' ? v.length : 0), 0) > 8 * 1024 * 1024)
        return { ...missing, warnings: ['Combined backpack encoding exceeds its size limit.'] };
    const results = await new Promise(resolve => {
        const worker = new Worker(new URL('./backpack-worker.js', import.meta.url), { workerData: encoded, env: {}, execArgv: [], resourceLimits: { maxOldGenerationSizeMb: 64, stackSizeMb: 2 } });
        const fail = () => { clearTimeout(timer); void worker.terminate(); resolve(entries.map(() => null)); };
        const timer = setTimeout(fail, 10_000);
        worker.once('message', (v) => { clearTimeout(timer); void worker.terminate(); resolve(v); });
        worker.once('error', fail);
        worker.once('exit', () => { clearTimeout(timer); resolve(entries.map(() => null)); });
    });
    const items = [], containers = [], warnings = [];
    entries.forEach(([id], i) => {
        const decoded = results[i] ?? null;
        containers.push({ containerId: id, status: decoded === null ? 'unavailable' : 'available', itemCount: decoded?.length ?? null });
        if (decoded === null)
            warnings.push(`Backpack ${id}: contents could not be decoded within safety limits.`);
        else
            items.push(...decoded.map(item => ({ ...item, containerId: id })));
    });
    const known = containers.filter(c => c.status === 'available').length;
    return { status: known === entries.length ? 'available' : known ? 'partial' : 'unavailable', items: known ? items : null, containers, warnings };
}
