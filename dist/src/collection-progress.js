import { at, number, object, text } from './types.js';
/** Official collection resource thresholds plus explicitly reported member totals.
 * These are lifetime collections, never spendable physical inventory. */
export function collectionProgress(member, resource) {
    const groups = object(resource.collections), definitions = new Map();
    for (const [category, group] of Object.entries(groups))
        for (const [id, v] of Object.entries(object(at(group, 'items'))))
            definitions.set(id, { ...object(v), category });
    const amounts = object(member.collection), unlocks = at(member, 'player_data', 'unlocked_coll_tiers'), ids = new Set([...definitions.keys(), ...Object.keys(amounts)]);
    const rows = [...ids].sort().map(id => {
        const definition = definitions.get(id), raw = number(amounts[id]), amount = raw !== null && raw >= 0 ? raw : null;
        const tiers = Array.isArray(definition?.tiers) ? definition.tiers.map(v => ({ tier: number(at(v, 'tier')), amountRequired: number(at(v, 'amountRequired')), unlocks: Array.isArray(at(v, 'unlocks')) ? at(v, 'unlocks').map(x => text(x, 256)).filter(x => x !== null) : null })).filter(v => v.tier !== null && v.amountRequired !== null).sort((a, b) => a.amountRequired - b.amountRequired) : [];
        const reached = amount === null ? null : tiers.filter(t => amount >= t.amountRequired).at(-1)?.tier ?? (tiers.length ? 0 : null), next = amount === null ? null : tiers.find(t => amount < t.amountRequired) ?? null;
        const reported = Array.isArray(unlocks) ? unlocks.filter(v => typeof v === 'string' && v.startsWith(id + '_')) : null;
        return { id, name: text(definition?.name), category: definition?.category ?? null, amount, unit: 'items', scope: 'member', valueKind: 'lifetime_collection', status: amount === null ? 'unknown' : tiers.length ? 'known' : 'partial', reasonCode: amount === null ? 'member_amount_not_exposed' : tiers.length ? null : 'official_collection_definition_unavailable', tierByMemberAmount: reached, reportedUnlockedTiers: reported, coOpTier: null, coOpReasonCode: 'other_members_not_collected_or_combined', nextTier: next, remainingToNextTier: amount !== null && next ? Math.max(0, next.amountRequired - amount) : null, maximumTier: number(definition?.maxTiers), tiers, physicalResourcesAvailable: null };
    });
    return { status: member.collection === undefined ? 'unavailable' : definitions.size ? 'partial' : 'partial', data: { collected: Object.fromEntries(Object.entries(amounts).filter(([, v]) => number(v) !== null)), progress: rows, unlockedRecipes: at(member, 'player_data', 'unlocked_coll_tiers') ?? null, coverage: { returned: rows.length, officialDefinitions: definitions.size, reportedAmounts: Object.keys(amounts).length, allMemberAmountsKnown: rows.every(r => r.amount !== null), coOpCollectionsKnown: false }, resourceLastUpdated: resource.lastUpdated ?? null }, warnings: ['A missing member collection is unknown, not zero. Official tier thresholds applied to member totals do not establish shared co-op recipe eligibility. Lifetime amounts are distinct from spendable materials.'] };
}
