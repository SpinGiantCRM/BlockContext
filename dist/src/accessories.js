import { at, object, text } from './types.js';
import { DOMAIN_REVISION, IGNORED_ACCESSORIES, KNOWN_ACCESSORIES, MAGICAL_POWER, UPGRADE_RELATIONS } from './domain.js';
export function describeItems(items, catalog) {
    return items?.map(item => {
        const definition = object(at(catalog, 'items', item.id ?? ''));
        const category = text(definition.category);
        return { ...item, category: category ?? item.loreCategory, categorySource: category !== null ? 'hypixel_item_catalog' : item.loreCategory !== null ? 'inventory_footer' : null, catalogTier: text(definition.tier), officialDefinition: Object.keys(definition).length ? definition : null, requirements: definition.requirements ?? null, baseStats: definition.stats ?? null, npcSellPrice: definition.npcSellPrice ?? null, canAuction: typeof definition.can_auction === 'boolean' ? definition.can_auction : null, canTrade: typeof definition.can_trade === 'boolean' ? definition.can_trade : null, raritySource: item.rarity === null ? null : 'inventory_footer' };
    }) ?? null;
}
export function magicalPower(bag, member) {
    const reasons = [], excluded = [], active = [];
    if (bag === null)
        return { currentMagicalPower: null, calculatedMagicalPower: null, calculationStatus: 'unavailable', reasons: ['Accessory bag is unavailable.'], activeItems: null, suppressedItems: null, domainRevision: DOMAIN_REVISION };
    // Reciprocal relations are aliases; directed relations are upgrade paths.
    const alias = (id) => [id, ...(UPGRADE_RELATIONS[id] ?? []).filter(other => UPGRADE_RELATIONS[other]?.includes(id))].sort()[0];
    const candidates = bag.filter(item => {
        if (typeof item.id !== 'string' || item.category !== 'ACCESSORY' || typeof item.rarity !== 'string' || MAGICAL_POWER[item.rarity] === undefined) {
            reasons.push(`Slot ${item.slot}: accessory identity, category or effective rarity is unknown.`);
            return false;
        }
        if (!KNOWN_ACCESSORIES.has(item.id)) {
            reasons.push(`Slot ${item.slot}: item ${item.id} is newer than or absent from the supported accessory rules.`);
            return false;
        }
        if (IGNORED_ACCESSORIES.has(item.id) || item.id === 'RIFT_PRISM') {
            excluded.push({ id: item.id, slot: item.slot, reason: item.id === 'RIFT_PRISM' ? 'Prism power requires consumption, not possession.' : 'Rift-only or ignored accessory.' });
            return false;
        }
        return true;
    });
    const groups = new Map();
    for (const item of candidates) {
        const id = alias(String(item.id));
        groups.set(id, [...(groups.get(id) ?? []), item]);
    }
    const unique = [];
    for (const entries of groups.values()) {
        entries.sort((a, b) => MAGICAL_POWER[String(b.rarity)] - MAGICAL_POWER[String(a.rarity)] || Number(a.slot) - Number(b.slot));
        unique.push(entries[0]);
        for (const item of entries.slice(1))
            excluded.push({ id: item.id, slot: item.slot, reason: 'Duplicate or alias; the highest effective rarity wins.', replacedBy: entries[0].id });
    }
    for (const item of unique) {
        const stronger = unique.filter(other => other !== item && alias(String(other.id)) !== alias(String(item.id)) && (UPGRADE_RELATIONS[String(item.id)] ?? []).includes(String(other.id)) && !(UPGRADE_RELATIONS[String(other.id)] ?? []).includes(String(item.id)));
        if (stronger.length) {
            if (stronger.every(other => MAGICAL_POWER[String(other.rarity)] < MAGICAL_POWER[String(item.rarity)]))
                reasons.push(`Upgrade path ${item.id}: lower-tier recombination exceeds the higher tier's rarity; effective precedence needs verification.`);
            excluded.push({ id: item.id, slot: item.slot, reason: 'Higher upgrade tier is present in the accessory bag.', replacedBy: stronger.map(i => i.id) });
        }
        else
            active.push(item);
    }
    const contributions = active.map(item => ({ id: item.id, slot: item.slot, rarity: item.rarity, magicalPower: MAGICAL_POWER[String(item.rarity)] * (item.id === 'HEGEMONY_ARTIFACT' ? 2 : 1) }));
    const base = contributions.reduce((n, i) => n + i.magicalPower, 0);
    let abicase = 0;
    if (active.some(i => i.id === 'ABICASE')) {
        const contacts = at(member, 'nether_island_player_data', 'abiphone', 'active_contacts');
        if (!Array.isArray(contacts) || contacts.length > 256 || contacts.some(v => typeof v !== 'string'))
            reasons.push('Abicase contact bonus is unavailable.');
        else
            abicase = Math.floor(new Set(contacts).size / 2);
    }
    const prism = at(member, 'rift', 'access', 'consumed_prism');
    if (prism !== undefined && typeof prism !== 'boolean')
        reasons.push('Rift Prism consumption flag is invalid.');
    const prismBonus = prism === true ? 11 : 0;
    const calculated = base + abicase + prismBonus;
    return { currentMagicalPower: reasons.length ? null : calculated, calculatedMagicalPower: calculated, calculationStatus: reasons.length ? 'partial' : 'available', reasons, scope: 'Accessory bag only, outside Dungeons; carried and stored accessories do not contribute.', breakdown: { accessories: base, abicaseContacts: abicase, consumedRiftPrism: prismBonus }, activeItems: contributions, suppressedItems: excluded, domainRevision: DOMAIN_REVISION, interpretation: 'Calculated from this API snapshot and pinned community rules; not a live in-game stat readout. Unknown rarities and bonuses prevent a complete total.' };
}
export function accessoryDiagnostics(bag, carried) {
    const groups = new Map();
    for (const item of bag ?? [])
        if (typeof item.id === 'string')
            groups.set(item.id, [...(groups.get(item.id) ?? []), item]);
    const knownCarried = carried?.filter(item => item.category === 'ACCESSORY') ?? null;
    const duplicates = bag === null ? null : [...groups].filter(([, items]) => items.length > 1).map(([id, items]) => ({ id, occupiedSlots: items.length, slots: items.map(i => i.slot) }));
    return {
        bagItemCount: bag?.length ?? null,
        distinctBagItemIds: bag === null ? null : groups.size,
        unknownBagItemIds: bag === null ? null : bag.filter(i => i.id === null).length,
        duplicateItemIds: duplicates,
        carriedAccessories: knownCarried,
        carriedAccessoriesAbsentFromBag: bag === null ? null : knownCarried?.filter(i => typeof i.id === 'string' && !groups.has(i.id)) ?? null,
    };
}
