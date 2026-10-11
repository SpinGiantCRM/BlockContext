import { at, object } from './types.js';
const LIMIT = 256;
function coverage(status, reasonCode, source) { return { status, reasonCode, source }; }
function effects(raw, source) {
    if (!Array.isArray(raw))
        return { items: null, count: null, knownCount: null, coverage: coverage('unavailable', raw === undefined ? 'not_exposed' : 'invalid_api_value', source) };
    let invalidEntries = 0, incompleteEntries = 0;
    const items = [];
    for (const value of raw.slice(0, LIMIT)) {
        const p = object(value);
        if (typeof p.effect !== 'string' || !/^[a-zA-Z0-9_:-]{1,64}$/.test(p.effect)) {
            invalidEntries++;
            continue;
        }
        const level = Number.isSafeInteger(p.level) && Number(p.level) > 0 ? Number(p.level) : null;
        const infinite = typeof p.infinite === 'boolean' ? p.infinite : null;
        const durationTicks = infinite === true ? null : Number.isSafeInteger(p.duration_ticks) && Number(p.duration_ticks) >= 0 ? Number(p.duration_ticks) : null;
        const fieldCoverage = {
            level: coverage(level === null ? 'unavailable' : 'available', level === null ? (p.level === undefined ? 'not_exposed' : 'invalid_api_value') : null, `${source}.level`),
            durationTicks: coverage(durationTicks === null ? 'unavailable' : 'available', infinite === true ? 'infinite_effect' : durationTicks === null ? (p.duration_ticks === undefined ? 'not_exposed' : 'invalid_api_value') : null, `${source}.duration_ticks`),
            infinite: coverage(infinite === null ? 'unavailable' : 'available', infinite === null ? (p.infinite === undefined ? 'not_exposed' : 'invalid_api_value') : null, `${source}.infinite`)
        };
        const complete = level !== null && (durationTicks !== null || infinite === true) && infinite !== null;
        if (!complete)
            incompleteEntries++;
        items.push({ effect: p.effect, level, durationTicks, infinite, status: complete ? 'available' : 'partial', fieldCoverage });
    }
    const truncated = raw.length > LIMIT, partial = truncated || invalidEntries > 0 || incompleteEntries > 0;
    return { items, count: partial ? null : items.length, knownCount: items.length, coverage: { ...coverage(partial ? 'partial' : 'available', truncated ? 'safety_limit' : invalidEntries ? 'invalid_effect_entries' : incompleteEntries ? 'missing_or_invalid_effect_fields' : null, source), invalidEntries, incompleteEntries, truncated } };
}
export function buffs(member) {
    const cookie = at(member, 'profile', 'cookie_buff_active'), active = typeof cookie === 'boolean' ? cookie : null;
    const activeEffects = effects(at(member, 'player_data', 'active_effects'), 'member.player_data.active_effects');
    const pausedEffects = effects(at(member, 'player_data', 'paused_effects'), 'member.player_data.paused_effects');
    const cookieCoverage = coverage(active === null ? 'unavailable' : 'available', active === null ? (cookie === undefined ? 'not_exposed' : 'invalid_api_value') : null, 'member.profile.cookie_buff_active');
    return {
        anyAvailable: active !== null || activeEffects.items !== null || pausedEffects.items !== null,
        incomplete: active === null || activeEffects.coverage.status !== 'available' || pausedEffects.coverage.status !== 'available',
        data: {
            boosterCookie: { active, remainingSeconds: null }, godPotion: { active: null, remainingSeconds: null },
            activePotionEffects: activeEffects.items, pausedPotionEffects: pausedEffects.items,
            activePotionEffectCount: activeEffects.count, pausedPotionEffectCount: pausedEffects.count,
            knownActivePotionEffectCount: activeEffects.knownCount, knownPausedPotionEffectCount: pausedEffects.knownCount,
            potionTiming: 'Saved API tick values, not a live countdown. No wall-clock expiry is inferred.'
        },
        fieldCoverage: {
            boosterCookieActive: cookieCoverage,
            boosterCookieRemainingSeconds: coverage('unavailable', 'not_exposed_as_verified_timer', 'Hypixel profile API'),
            godPotionActive: coverage('unavailable', 'not_exposed_as_verified_status', 'Hypixel profile API'),
            godPotionRemainingSeconds: coverage('unavailable', 'not_exposed_as_verified_timer', 'Hypixel profile API'),
            activePotionEffects: activeEffects.coverage, pausedPotionEffects: pausedEffects.coverage
        },
        warnings: ['Cookie and potion effects are API-reported snapshots; cache age does not prove their current in-game state. Dedicated God Potion status and cookie/God Potion timers remain unknown; effect lists do not establish a God Potion or its expiry.',
            ...(active === null || activeEffects.coverage.status !== 'available' || pausedEffects.coverage.status !== 'available' ? ['Some cookie or potion-effect fields are missing, malformed or exceed safety limits; unknown does not mean inactive or an empty list.'] : [])]
    };
}
