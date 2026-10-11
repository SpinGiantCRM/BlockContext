import { Worker } from 'node:worker_threads';
import { at, object } from './types.js';
export async function loadouts(value, kind) {
    const fail = (reasonCode) => ({ status: 'unavailable', items: null, sets: null, reasonCode, warnings: [`${kind} loadouts: ${reasonCode}. Missing contents do not mean empty storage.`] });
    if (value === undefined)
        return fail('not_exposed');
    if (value === null || typeof value !== 'object' || Array.isArray(value))
        return fail('invalid_api_value');
    const entries = Object.entries(object(value)).filter(([k]) => k !== 'equipped_set');
    if (entries.length > 36 || entries.some(([k]) => !/^(0|[1-9][0-9]{0,2})$/.test(k)))
        return fail('safety_limit_or_invalid_set_id');
    const pieces = kind === 'armor' ? ['HELMET', 'CHESTPLATE', 'LEGGINGS', 'BOOTS'] : ['EQUIPMENT_SLOT_1', 'EQUIPMENT_SLOT_2', 'EQUIPMENT_SLOT_3', 'EQUIPMENT_SLOT_4'];
    entries.sort(([a], [b]) => Number(a) - Number(b));
    const tasks = [], invalid = new Set();
    for (const [id, v] of entries) {
        if (v === null || typeof v !== 'object' || Array.isArray(v) || (at(v, 'id') !== undefined && at(v, 'id') !== Number(id)) || Object.keys(object(v)).some(k => k !== 'id' && !pieces.includes(k))) {
            invalid.add(id);
            continue;
        }
        // The API omits empty pieces, including entire unused sets with only an id.
        for (const piece of pieces)
            if (at(v, piece) !== undefined)
                tasks.push({ setId: id, piece, data: at(v, piece, 'data') });
    }
    if (tasks.reduce((n, t) => n + (typeof t.data === 'string' ? t.data.length : 0), 0) > 8 * 1024 * 1024)
        return fail('safety_limit');
    const results = tasks.length ? await new Promise(resolve => {
        const worker = new Worker(new URL('./backpack-worker.js', import.meta.url), { workerData: tasks.map(t => t.data), env: {}, execArgv: [], resourceLimits: { maxOldGenerationSizeMb: 64, stackSizeMb: 2 } });
        const failure = () => { clearTimeout(timer); void worker.terminate(); resolve(tasks.map(() => null)); };
        const timer = setTimeout(failure, 10_000);
        worker.once('message', (v) => { clearTimeout(timer); void worker.terminate(); resolve(v); });
        worker.once('error', failure);
        worker.once('exit', () => { clearTimeout(timer); resolve(tasks.map(() => null)); });
    }) : [];
    const items = [], unknownPieces = new Map();
    tasks.forEach((task, i) => {
        const decoded = results[i];
        if (!decoded || decoded.length > 1 || decoded.some(item => item.id === null || item.count !== 1)) {
            invalid.add(task.setId);
            unknownPieces.set(task.setId, [...unknownPieces.get(task.setId) ?? [], task.piece]);
        }
        else
            items.push(...decoded.map(item => ({ ...item, setId: task.setId, piece: task.piece })));
    });
    const status = invalid.size ? (invalid.size === entries.length && !items.length ? 'unavailable' : 'partial') : 'available';
    return { status, items: status === 'unavailable' ? null : items, sets: entries.map(([id]) => ({ setId: id, status: invalid.has(id) ? 'partial' : 'available', itemCount: invalid.has(id) ? null : items.filter(i => i.setId === id).length, knownItemCount: items.filter(i => i.setId === id).length, unknownPieces: unknownPieces.get(id) ?? [], equipped: at(value, 'equipped_set') === Number(id) })), reasonCode: invalid.size ? 'decode_or_invalid_api_value' : null, warnings: invalid.size ? [`${kind} loadouts contain unreadable pieces; complete item counts are unknown.`] : [] };
}
