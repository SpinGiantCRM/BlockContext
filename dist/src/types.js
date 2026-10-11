export const SECTIONS = ['stats', 'skills', 'currencies', 'slayers', 'dungeons', 'pets', 'collections', 'minions', 'equipment', 'accessories', 'garden', 'mining', 'foraging', 'hunting', 'dailies', 'quests', 'rift', 'crimson_isle', 'chocolate_factory', 'fishing', 'bestiary', 'travel', 'profile_upgrades', 'events', 'orders', 'museum', 'api_fields', 'storage', 'data_contract', 'systems'];
export class AppError extends Error {
    code;
    transient;
    retryAfterSeconds;
    constructor(code, message, transient = false, retryAfterSeconds) {
        super(message);
        this.code = code;
        this.transient = transient;
        this.retryAfterSeconds = retryAfterSeconds;
    }
}
export function object(value) {
    return value !== null && typeof value === 'object' && !Array.isArray(value) ? value : {};
}
export function at(value, ...path) {
    for (const key of path)
        value = object(value)[key];
    return value;
}
export function number(value) { return typeof value === 'number' && Number.isFinite(value) ? value : null; }
export function text(value, max = 96) {
    return typeof value === 'string' ? value.replace(/§[0-9a-fk-or]/gi, '').replace(/[\u0000-\u001f\u007f]/g, '').slice(0, max) : null;
}
export function numericMap(value, limit = 2048) {
    return Object.fromEntries(Object.entries(object(value)).slice(0, limit).filter(([k, v]) => /^[a-zA-Z0-9_:.-]{1,96}$/.test(k) && number(v) !== null));
}
