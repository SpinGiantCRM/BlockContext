import { at, object, text } from './types.js';
export const DOMAIN_ROOTS = {
    stats: ['player_stats', 'fairy_soul', 'leveling', 'player_data'], skills: ['player_data.experience'], currencies: ['currencies', 'profile.bank_account', 'item_data', 'events.easter.chocolate'],
    slayers: ['slayer'], dungeons: ['dungeons'], pets: ['pets_data'], collections: ['collection', 'player_data.unlocked_coll_tiers'], minions: ['player_data.crafted_generators', 'crafted_generators'],
    equipment: ['inventory', 'shared_inventory', 'loadout', 'item_data'], accessories: ['accessory_bag_storage'], garden: ['garden_player_data', 'jacobs_contest'], mining: ['mining_core', 'glacite_player_data', 'forge', 'skill_tree'],
    foraging: ['foraging', 'foraging_core', 'skill_tree'], hunting: ['shards', 'attributes', 'temples', 'foraging.hunting_toolkit'], quests: ['quests', 'objectives', 'leveling'], rift: ['rift'], crimson_isle: ['nether_island_player_data'],
    chocolate_factory: ['events.easter'], fishing: ['trophy_fish', 'player_stats'], bestiary: ['bestiary'], travel: ['player_data.visited_zones', 'player_data.visited_modes', 'player_data.fast_travel'], profile_upgrades: ['profile'], events: ['events', 'winter_player_data'], orders: ['auction', 'bazaar'], museum: ['museum']
};
const privateKey = /(?:^|_)(?:private|uuid|uniqueid|player_id|player_uuid|email|ip|token|secret|password|session|sessions|chat|messages|social_media|coop_invitation|deletion_notice|last_login|last_logout|first_login|first_join|participants|teammates|started_by|claimed_by|who_started|initiator|initiator_name|owner|owners|donator|originator)(?:$|_)/i;
const uuid = /^(?:[a-f0-9]{32}|[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12})$/i;
// Numeric game counters are not credentials. Keep this exception exact so a
// similarly named text field cannot bypass the private-field filter.
function privateField(key, value) {
    if (typeof value === 'number' && Number.isFinite(value) && ['secret_count', 'secrets_found', 'token_count'].includes(key))
        return false;
    return privateKey.test(key);
}
export function stampObservedAt(value, retrievedAt) {
    if (value === null || typeof value !== 'object')
        return;
    for (const [key, child] of Object.entries(value)) {
        if (key === 'observedAt' && child === 'see_response_metadata.retrievedAt')
            value[key] = new Date(retrievedAt).toISOString();
        else
            stampObservedAt(child, retrievedAt);
    }
}
export function pathValue(root, path) { return path.split('.').reduce((v, k) => at(v, k), root); }
export function domainFor(path) {
    const p = path.replace(/^member\./, '');
    return Object.entries(DOMAIN_ROOTS).flatMap(([domain, roots]) => roots.filter(root => p === root || p.startsWith(root + '.')).map(root => ({ domain, length: root.length }))).sort((a, b) => b.length - a.length)[0]?.domain ?? 'other_game_systems';
}
export function fieldUnit(path) {
    if (/(?:coin_purse|bank_account|\.balance|\.amount_coins|coins_paid)$/.test(path))
        return 'SkyBlock coins';
    if (/(?:experience|\.xp|_xp|\.exp)$/.test(path))
        return 'XP';
    if (/(?:powder_(?:mithril|gemstone|glacite)(?:_total)?|powder_spent_(?:mithril|gemstone|glacite))$/.test(path))
        return 'powder';
    if (/(?:\.collection\.[A-Z0-9_:]+|\.amount_owned)$/.test(path))
        return 'items';
    if (/(?:\.current)$/.test(path) && path.includes('.essence.'))
        return 'essence';
    return null;
}
/** Game facts only, with field-level mapping and explicit omissions. No raw endpoint proxy. */
export function mapGameFields(value, prefix = 'member', scope = 'member', maxFields = 12000) {
    const fields = [], omissions = [];
    let nodes = 0, leaves = 0, truncated = false;
    const omit = (path, reason) => { if (omissions.length < 128)
        omissions.push({ path, reasonCode: reason }); };
    const visit = (v, path, depth) => {
        if (++nodes > 100000 || depth > 20) {
            truncated = true;
            omit(path, 'projection_budget');
            return;
        }
        const key = path.split('.').at(-1);
        if (privateField(key, v) || uuid.test(key) || /^\d{1,3}(?:\.\d{1,3}){3}$/.test(key)) {
            omit(path, 'identity_or_non_game_private_field');
            return;
        }
        if (v !== null && typeof v === 'object' && !Array.isArray(v) && typeof at(v, 'data') === 'string' && /^[A-Za-z0-9+/]+=*$/.test(String(at(v, 'data')))) {
            if (fields.length >= maxFields) {
                truncated = true;
                return;
            }
            leaves++;
            fields.push({ path, status: 'partial', value: null, valueType: 'encoded_inventory', domain: domainFor(path), scope, unit: null, observedAt: 'see_response_metadata.retrievedAt', gameUpdatedAt: null, meaning: 'Encoded item container; see storage.items and decoded equipment locations.', reasonCode: 'encoded_inventory_not_exported' });
            return;
        }
        if (v !== null && typeof v === 'object') {
            const entries = Array.isArray(v) ? v.map((x, i) => [String(i), x]) : Object.entries(object(v));
            if (entries.length === 0) {
                leaves++;
                if (fields.length < maxFields)
                    fields.push({ path, status: 'known', value: Array.isArray(v) ? [] : {}, valueType: Array.isArray(v) ? 'array' : 'object', domain: domainFor(path), scope, unit: null, observedAt: 'see_response_metadata.retrievedAt', gameUpdatedAt: null, meaning: 'Explicit empty game field; not inferred from absence.' });
                else
                    truncated = true;
            }
            for (const [k, x] of entries)
                if (/^[A-Za-z0-9_: .+-]{1,96}$/.test(k))
                    visit(x, path + '.' + k, depth + 1);
                else
                    omit(path + '.[invalid_key]', 'invalid_key');
            return;
        }
        leaves++;
        if (fields.length >= maxFields) {
            truncated = true;
            return;
        }
        if (typeof v === 'string' && (uuid.test(v) || v.includes('@') || v.length > 512 || /(?:Bearer |sk-[A-Za-z0-9]|-----BEGIN)/.test(v))) {
            omit(path, 'identity_private_or_unbounded_text');
            return;
        }
        if (typeof v === 'number' && !Number.isFinite(v)) {
            omit(path, 'invalid_number');
            return;
        }
        if (v !== null && !['string', 'boolean', 'number'].includes(typeof v)) {
            omit(path, 'invalid_type');
            return;
        }
        const unit = fieldUnit(path), knownMeaning = unit !== null;
        fields.push({ path, status: v === null ? 'unknown' : 'known', value: typeof v === 'string' ? text(v, 512) : v, valueType: v === null ? 'null' : typeof v, domain: domainFor(path), scope, unit, meaning: knownMeaning ? 'Reported ' + unit + ' value.' : 'Reported game field; exact unit or game semantics are not inferred.', observedAt: 'see_response_metadata.retrievedAt', gameUpdatedAt: null, semanticStatus: knownMeaning ? 'unit_mapped' : 'reported_field_semantics_unverified', reasonCode: v === null ? 'api_returned_null' : null });
    };
    visit(value, prefix, 0);
    return { fields, coverage: { returned: fields.length, visitedLeafFields: leaves, omissions, omissionsLimited: omissions.length === 128, truncated, completeGameFieldEnumeration: !truncated, privateIdentitiesExcluded: true, encodedInventoriesExported: false, allSemanticsVerified: false } };
}
/** Keep own game payload for transforms, stripping identities while preserving encoded own inventories. */
export function projectOwnGame(value, depth = 0) {
    if (depth > 32)
        throw new Error('Game projection exceeds the supported nesting depth');
    if (value === null || typeof value === 'boolean' || typeof value === 'number')
        return typeof value === 'number' && !Number.isFinite(value) ? null : value;
    if (typeof value === 'string')
        return uuid.test(value) || value.includes('@') ? null : value;
    if (Array.isArray(value))
        return value.map(v => projectOwnGame(v, depth + 1));
    return Object.fromEntries(Object.entries(object(value)).filter(([k, v]) => !privateField(k, v) && !uuid.test(k) && /^[A-Za-z0-9_: .+-]{1,96}$/.test(k)).map(([k, v]) => [k, projectOwnGame(v, depth + 1)]));
}
export function additionalSections(member, raw, metadata) {
    const mapped = mapGameFields(member), shared = mapGameFields(Object.fromEntries(Object.entries(raw).filter(([k]) => !['members', 'profile_id', 'cute_name', 'selected', 'game_mode'].includes(k))), 'profile', 'shared_profile');
    const account = mapGameFields(metadata, 'account', 'account');
    const all = [...mapped.fields, ...shared.fields, ...account.fields];
    const sections = { api_fields: { status: 'partial', data: { fields: all, coverage: { member: mapped.coverage, sharedProfile: shared.coverage, account: account.coverage }, newFieldsRetained: true }, warnings: ['Every retained game field is mapped by source path, type and scope. Undocumented semantics stay unverified; identities and encoded inventory payloads are excluded.'] } };
    for (const domain of ['fishing', 'bestiary', 'travel', 'profile_upgrades', 'events', 'orders', 'museum']) {
        const fields = all.filter(f => f.domain === domain || domain === 'profile_upgrades' && String(f.path).startsWith('profile.community_upgrades'));
        sections[domain] = { status: fields.length ? 'partial' : 'unavailable', data: { fields, complete: false, reasonCode: fields.length ? 'reported_game_fields' : 'not_exposed_in_profile_snapshot' }, warnings: ['No absence is interpreted as zero, empty holdings, locked or uncompleted. Separate endpoints and manually viewed menus may provide more evidence.'] };
    }
    return sections;
}
