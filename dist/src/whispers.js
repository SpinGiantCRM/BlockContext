import { at, number, object } from './types.js';
import { field } from './game-systems.js';
const source = 'https://github.com/SkyblockerMod/Skyblocker/blob/32ed634695a54f97cf8ae2dc234bd7bedfbfcc2f/src/main/java/de/hysky/skyblocker/skyblock/profileviewer2/model/ForagingCore.java';
function counter(v) { const n = number(v); return n !== null && Number.isSafeInteger(n) && n >= 0 ? n : null; }
/** Modern counters are per preset. Their presence does not establish the active
 * preset, or whether non-refundable spend is included in either other counter. */
export function whisperBalances(member) {
    const selected = counter(at(member, 'skill_tree', 'selected_skill_tree_slot', 'foraging'));
    const modern = at(member, 'foraging_core', 'whispers');
    const currencies = Object.fromEntries(['forest', 'desert'].map(kind => {
        const root = object(at(modern, kind)), path = 'member.foraging_core.whispers.' + kind;
        const presets = Object.entries(root).filter(([key]) => /^[1-5]$/.test(key)).map(([preset, v]) => ({ preset: Number(preset), spent: field(counter(at(v, 'spent')), path + '.' + preset + '.spent', kind + ' Whispers'), spentNonRefundable: field(counter(at(v, 'spent_non_refundable')), path + '.' + preset + '.spent_non_refundable', kind + ' Whispers') }));
        const active = presets.find(p => p.preset === selected);
        const reportedTotal = field(counter(root.total), path + '.total', kind + ' Whispers');
        return [kind, { reportedTotal: { ...reportedTotal, meaning: 'API counter named total; spendable-balance arithmetic is not yet verified.' }, presets, selectedPreset: field(selected, 'member.skill_tree.selected_skill_tree_slot.foraging', 'preset'), spent: active?.spent ?? { ...field(null, path + '.[selected_preset].spent', kind + ' Whispers'), reasonCode: 'selected_preset_not_exposed_or_spending_missing' }, available: { ...field(null, path, kind + ' Whispers'), reasonCode: 'modern_total_and_spend_semantics_not_verified' }, semanticStatus: 'reported_counters_only', reference: { source, liveGameVerified: false } }];
    }));
    const legacyAvailable = counter(at(member, 'foraging_core', 'forests_whispers')), legacySpent = counter(at(member, 'foraging_core', 'forests_whispers_spent'));
    const forest = currencies.forest;
    return { schema: modern !== undefined ? 'modern_per_preset' : 'legacy', forest, desert: currencies.desert,
        available: modern !== undefined ? forest.available : field(legacyAvailable, 'member.foraging_core.forests_whispers', 'Forest Whispers'),
        spent: modern !== undefined ? forest.spent : field(legacySpent, 'member.foraging_core.forests_whispers_spent', 'Forest Whispers'),
        total: modern !== undefined ? forest.reportedTotal : field(legacyAvailable !== null && legacySpent !== null ? legacyAvailable + legacySpent : null, 'calculated_from_verified_legacy_available_and_spent', 'Forest Whispers'),
        reconciliation: { status: 'partial', crossSourceEqualityVerified: false, reasonCode: modern !== undefined ? 'modern_counters_need_displayed_balance_verification' : 'legacy_counter_reference_only' },
        legacyReported: { available: field(legacyAvailable, 'member.foraging_core.forests_whispers', 'Forest Whispers'), spent: field(legacySpent, 'member.foraging_core.forests_whispers_spent', 'Forest Whispers') } };
}
/** Preserve API counters and separately identify what is actually displayed now. */
export function withDisplayedWhispers(api, client) {
    const reports = [];
    for (const [key, v] of Object.entries(object(client.observations))) {
        if (!['foraging', 'tabStats'].includes(key))
            continue;
        const row = object(v), facts = at(row, 'values', 'facts');
        if (!Array.isArray(facts))
            continue;
        for (const fact of facts)
            for (const f of Array.isArray(at(fact, 'fields')) ? at(fact, 'fields') : []) {
                if (!['Forest Whispers', 'Desert Whispers'].includes(String(f.label)))
                    continue;
                const value = counter(f.value);
                if (value === null)
                    continue;
                reports.push({ currency: f.label === 'Forest Whispers' ? 'forest' : 'desert', value, unit: f.label, status: row.currentStateVerified === true ? 'known' : 'stale', source: 'client_observation', sourceKey: key, observedAt: row.observedAt, currentStateVerified: row.currentStateVerified === true });
            }
    }
    const balance = (kind, previous) => {
        const all = reports.filter(r => r.currency === kind), current = all.filter(r => r.currentStateVerified === true), values = new Set(current.map(r => r.value));
        if (values.size === 1)
            return { ...current[0], scope: 'member', reasonCode: null };
        if (values.size > 1)
            return { ...field(null, 'client.foraging', kind + ' Whispers'), reasonCode: 'conflicting_current_displayed_balances' };
        if (object(previous).status === 'known')
            return previous;
        const latest = [...all].sort((a, b) => String(b.observedAt).localeCompare(String(a.observedAt)))[0];
        if (!latest)
            return previous;
        const sameTime = all.filter(r => r.observedAt === latest.observedAt);
        if (new Set(sameTime.map(r => r.value)).size > 1)
            return { ...field(null, 'client.foraging', kind + ' Whispers'), reasonCode: 'conflicting_saved_displayed_balances' };
        return { ...latest, status: 'stale', scope: 'member', currentStateVerified: false, reasonCode: 'last_displayed_balance_not_verified_current' };
    };
    const available = balance('forest', api.available), forest = object(api.forest), desert = object(api.desert);
    return { ...api, available, apiAvailable: api.available, forest: { ...forest, available: balance('forest', forest.available), apiAvailable: forest.available }, desert: { ...desert, available: balance('desert', desert.available), apiAvailable: desert.available }, displayedBalances: reports, reconciliation: { ...object(api.reconciliation), displayedBalanceObserved: reports.length > 0, currentDisplayedBalanceVerified: reports.some(r => r.currentStateVerified === true), crossSourceEqualityVerified: false } };
}
