import { at } from './types.js';
function mutations(value) {
    if (!Array.isArray(value) || value.length > 256 || value.some(v => typeof v !== 'string' || !/^[a-z0-9_]{1,64}$/.test(v)))
        return null;
    return [...new Set(value)].sort();
}
export function greenhouse(member) {
    const discovered = mutations(at(member, 'garden_player_data', 'discovered_greenhouse_crops'));
    const analyzed = mutations(at(member, 'garden_player_data', 'analyzed_greenhouse_crops'));
    const warnings = ['Greenhouse mutations belong to the requested player, not the shared Garden. Missing lists are unknown, not proof that the Greenhouse is locked or that no mutations exist. Live layout, water, growth and harvest timing are not exposed by this tool.'];
    if (discovered === null || analyzed === null)
        warnings.push('One or both mutation lists are unavailable or invalid; complete progression cannot be established.');
    if (discovered !== null && analyzed?.some(id => !discovered.includes(id)))
        warnings.push('Some analyzed mutations are absent from the discovered list; the API lists are inconsistent.');
    return { status: discovered !== null && analyzed !== null && !analyzed.some(id => !discovered.includes(id)) ? 'available' : discovered === null && analyzed === null ? 'unavailable' : 'partial', data: {
            discoveredMutations: discovered, analyzedMutations: analyzed, discoveredCount: discovered?.length ?? null, analyzedCount: analyzed?.length ?? null,
            pendingAnalysis: discovered !== null && analyzed !== null ? discovered.filter(id => !analyzed.includes(id)) : null,
            layout: null, water: null, growth: null, harvestTiming: null
        }, warnings };
}
