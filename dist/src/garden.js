import { CROP_MILESTONES, DOMAIN_REVISION, GARDEN_EXPERIENCE } from './domain.js';
import { at, number, object, text } from './types.js';
export function thresholdProgress(amount, requirements, firstLevel = 0) {
    const xp = number(amount);
    if (xp === null || xp < 0 || requirements.some(n => !Number.isFinite(n) || n < 0))
        return { level: null, progress: null, toNext: null, maximumLevel: null };
    let level = firstLevel, total = 0;
    for (const needed of requirements) {
        if (xp < total + needed)
            return { level, progress: (xp - total) / needed, toNext: total + needed - xp, maximumLevel: firstLevel + requirements.length };
        total += needed;
        level++;
    }
    return { level, progress: null, toNext: null, maximumLevel: level };
}
function nullableMap(value, limit = 256) {
    if (value === null || typeof value !== 'object' || Array.isArray(value))
        return null;
    const entries = Object.entries(value);
    if (entries.length > limit || entries.some(([, v]) => number(v) === null || Number(v) < 0))
        return null;
    return Object.fromEntries(entries);
}
const cropAliases = { POTATO_ITEM: 'POTATO', CARROT_ITEM: 'CARROT', 'INK_SACK:3': 'COCOA_BEANS', NETHER_STALK: 'NETHER_WART', MUSHROOM_COLLECTION: 'MUSHROOM', DOUBLE_PLANT: 'SUNFLOWER' };
export function gardenSection(raw) {
    if (raw === null)
        return { status: 'unavailable', data: {}, warnings: ['Garden data is not available for this profile; this does not prove the Garden is locked.'] };
    const resources = nullableMap(raw.resources_collected), visits = nullableMap(at(raw, 'commission_data', 'visits')), completed = nullableMap(at(raw, 'commission_data', 'completed'));
    const plots = Array.isArray(raw.unlocked_plots_ids) ? [...new Set(raw.unlocked_plots_ids.slice(0, 64).map(v => text(v)).filter(v => v !== null))] : null;
    const commissions = raw.active_commissions !== undefined ? Object.entries(object(raw.active_commissions)).slice(0, 64).map(([id, value]) => ({ visitor: text(id), status: text(at(value, 'status')), requirements: Array.isArray(at(value, 'requirement')) ? at(value, 'requirement').slice(0, 32).map(r => ({ item: text(at(r, 'item')), amount: number(at(r, 'amount')), originalItem: text(at(r, 'original_item')), originalAmount: number(at(r, 'original_amount')) })) : null })) : null;
    const warnings = ['Garden data describes the shared profile. Crop counters are Garden milestone progress, not global collection totals. Composter values are the saved API snapshot, not a simulation of current production.'];
    const experience = number(raw.garden_experience) !== null && Number(raw.garden_experience) >= 0 ? Number(raw.garden_experience) : null;
    if (experience === null || resources === null || plots === null)
        warnings.push('Some Garden fields are unavailable and must not be interpreted as zero.');
    if (raw.greenhouse !== undefined)
        warnings.push('Greenhouse raw data is not exposed; unverified new fields are omitted.');
    return { status: experience === null || resources === null || plots === null ? 'partial' : 'available', data: {
            experience, levelProgress: thresholdProgress(experience, GARDEN_EXPERIENCE), domainRevision: DOMAIN_REVISION,
            resourcesCollected: resources,
            cropMilestones: resources === null ? null : Object.entries(resources).map(([id, amount]) => { const crop = cropAliases[id] ?? id; return { crop, resourceId: id, collected: amount, ...(CROP_MILESTONES[crop] ? thresholdProgress(amount, CROP_MILESTONES[crop]) : { level: null, progress: null, toNext: null, maximumLevel: null }) }; }),
            cropUpgradeLevels: nullableMap(raw.crop_upgrade_levels), gardenUpgradeLevels: nullableMap(raw.garden_upgrades),
            unlockedPlots: plots, unlockedPlotCount: plots?.length ?? null, barnSkin: text(raw.selected_barn_skin),
            visitors: { visitsByNpc: visits, completedByNpc: completed, totalVisits: visits === null ? null : Object.values(visits).reduce((n, v) => n + v, 0), totalCompleted: number(at(raw, 'commission_data', 'total_completed')) ?? (completed === null ? null : Object.values(completed).reduce((n, v) => n + v, 0)), uniqueServed: number(at(raw, 'commission_data', 'unique_npcs_served')) ?? (completed === null ? null : Object.values(completed).filter(v => v > 0).length) },
            activeVisitorRequests: commissions,
            composter: { organicMatter: number(at(raw, 'composter_data', 'organic_matter')), fuelUnits: number(at(raw, 'composter_data', 'fuel_units')), compostUnits: number(at(raw, 'composter_data', 'compost_units')), compostItems: number(at(raw, 'composter_data', 'compost_items')), conversionTicks: number(at(raw, 'composter_data', 'conversion_ticks')), lastSavedAt: number(at(raw, 'composter_data', 'last_save')), upgrades: nullableMap(at(raw, 'composter_data', 'upgrades')) }
        }, warnings };
}
