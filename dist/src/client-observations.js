import { open, constants } from 'node:fs/promises';
import { z } from 'zod';
import { AppError, SECTIONS } from './types.js';
const identifier = z.string().regex(/^[A-Z0-9_:.-]{1,96}$/);
const display = z.string().regex(/^[A-Za-z0-9 '():+./%✪★-]{1,96}$/);
const stored = z.object({ status: z.literal('partial'), displayName: z.string().regex(/^[A-Za-z][A-Za-z '-]{0,95}$/), stars: z.number().int().min(1).max(15).nullable(), completeWeaponDetails: z.literal(false) });
const privateAttribute = /(?:^|_)(?:uuid|uniqueid|player_id|player_uuid|owner|owners|donator|originator|email|ip|token|secret|password|session|chat|message|history)(?:$|_)/i;
const safeAttribute = (v) => v.split('.').every(k => /^[A-Za-z0-9_:-]{1,64}$/.test(k) && !privateAttribute.test(k) && !/^([a-f0-9]{32}|[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12})$/i.test(k));
const attributeValue = z.union([z.number().finite().min(-1e15).max(1e15), z.boolean(), z.string().regex(/^[A-Za-z0-9_ :.+/%,()'★✪-]{0,96}$/).refine(v => !/^([a-f0-9]{32}|[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12})$/i.test(v))]);
const attributes = z.object({ fields: z.array(z.object({ path: z.string().max(128).refine(safeAttribute), value: attributeValue, semanticStatus: z.literal('reported_attribute') })).max(128), omitted: z.number().int().nonnegative(), truncated: z.boolean(), complete: z.literal(false) });
const itemStats = z.array(z.object({ label: z.string().regex(/^[A-Za-z ]{1,40}$/), value: z.number().finite().min(-1e15).max(1e15), unit: z.enum(['stat', '%']), valueKind: z.literal('primary_displayed_item_stat') })).max(64);
const item = z.object({ structuredAttributes: attributes.optional(), receivedAttributeKeys: z.array(z.string().refine(safeAttribute)).max(128).optional(), displayedStats: itemStats.optional(), abilityScrolls: z.array(z.string().regex(/^[A-Z_]{1,96}$/)).max(16).optional(), attributes: z.record(z.string().regex(/^[a-zA-Z0-9_:.-]{1,96}$/), z.number().int().min(0).max(100)).refine(v => Object.keys(v).length <= 64).optional(), runes: z.record(z.string().regex(/^[a-zA-Z0-9_:.-]{1,96}$/), z.number().int().min(0).max(100)).refine(v => Object.keys(v).length <= 64).optional(), gems: z.record(z.string().regex(/^[A-Z0-9_]{1,64}$/), z.union([z.string().regex(/^[A-Z0-9_]{1,32}$/), z.object({ quality: z.string().regex(/^[A-Z0-9_]{1,32}$/).optional(), type: z.string().regex(/^[A-Z0-9_]{1,32}$/).optional() })])).refine(v => Object.keys(v).length <= 64).optional(), drill_part_engine: z.string().regex(/^[a-zA-Z0-9_:.-]{1,96}$/).optional(), drill_part_fuel_tank: z.string().regex(/^[a-zA-Z0-9_:.-]{1,96}$/).optional(), drill_part_upgrade_module: z.string().regex(/^[a-zA-Z0-9_:.-]{1,96}$/).optional(), skin: z.string().regex(/^[a-zA-Z0-9_:.-]{1,96}$/).optional(), art_of_peace_count: z.number().finite().min(0).max(1e15).optional(), etherwarp: z.number().finite().min(0).max(1e15).optional(), mana_disintegrator_count: z.number().finite().min(0).max(1e15).optional(), drill_fuel: z.number().finite().min(0).max(1e15).optional(), drill_fuel_capacity: z.number().finite().min(0).max(1e15).optional(), fuel: z.number().finite().min(0).max(1e15).optional(), pickonimbus_durability: z.number().finite().min(0).max(1e15).optional(), cultivating: z.number().finite().min(0).max(1e15).optional(), farmed_cultivating: z.number().finite().min(0).max(1e15).optional(), compact_blocks: z.number().finite().min(0).max(1e15).optional(), expertise_kills: z.number().finite().min(0).max(1e15).optional(), instanceId: z.string().regex(/^[a-f0-9]{64}$/).optional(), displayName: display.optional(), rarity: z.enum(['COMMON', 'UNCOMMON', 'RARE', 'EPIC', 'LEGENDARY', 'MYTHIC', 'DIVINE', 'SPECIAL', 'VERY SPECIAL']).optional(), itemType: z.string().regex(/^[A-Z ]{1,32}$/).optional(), storedWeapon: stored.optional(), slot: z.number().int().min(0).max(53), id: identifier, count: z.number().int().min(0).max(127), reforge: z.string().regex(/^[a-z0-9_]{1,64}$/).optional(), enchantments: z.record(z.string().regex(/^[a-z0-9_]{1,64}$/), z.number().int().min(1).max(100)).refine(v => Object.keys(v).length <= 128), hot_potato_count: z.number().int().min(0).max(100).optional(), rarity_upgrades: z.number().int().min(0).max(100).optional(), upgrade_level: z.number().int().min(0).max(100).optional(), dungeon_item_level: z.number().int().min(0).max(100).optional(), art_of_war_count: z.number().int().min(0).max(100).optional(), farming_for_dummies_count: z.number().int().min(0).max(100).optional(), wood_singularity_count: z.number().int().min(0).max(100).optional(), tuned_transmission: z.number().int().min(0).max(100).optional() });
const buff = z.object({ active: z.boolean(), remainingSecondsAtObservation: z.number().int().min(0).max(10 * 365 * 86400).nullable() });
const observation = z.object({ observedAt: z.number().int().nonnegative(), readFrom: z.enum(['tab_display', 'opened_menu', 'player_inventory', 'sidebar_display', 'action_bar', 'player_state']), values: z.unknown() });
const change = z.object({ path: z.string().regex(/^[A-Za-z0-9_: .()/\'-]{1,192}$/), previousValue: z.union([z.number().finite().min(-1e15).max(1e15), z.boolean(), z.string().regex(/^[A-Z0-9_:.-]{1,96}$/)]), value: z.union([z.number().finite().min(-1e15).max(1e15), z.boolean(), z.string().regex(/^[A-Z0-9_:.-]{1,96}$/)]), previousObservedAt: z.number().int().nonnegative(), observedAt: z.number().int().nonnegative(), kind: z.literal('observed_value_change') }).refine(c => c.previousObservedAt <= c.observedAt && !/(uuid|token|secret|password|email|chat|message)/i.test(c.path));
const profile = z.object({ uuid: z.string().regex(/^[a-f0-9]{32}$/), playerName: z.string().regex(/^[a-zA-Z0-9_]{1,16}$/), profileName: z.string().regex(/^[A-Za-z][A-Za-z ]{0,31}$/), lastSeenAt: z.number().int().nonnegative().optional(), changes: z.array(change).max(64).optional(), observations: z.record(z.string().max(96), observation).refine(v => Object.keys(v).length <= 96) });
const snapshot = z.object({ schemaVersion: z.literal(1), collectorVersion: z.enum(['0.1.0', '0.1.1', '0.2.0', '0.2.1', '0.2.2', '0.3.0']), exportedAt: z.number().int().nonnegative(), activeProfile: z.string().max(70).nullable(), profiles: z.array(profile).max(8), observedNow: z.array(z.string().max(96)).max(96).optional(), exportIntervalMs: z.number().int().min(1000).max(60000).optional(), evictedObservations: z.number().int().nonnegative().optional() });
const coins = z.number().finite().min(0).max(1e15);
const bankFields = ['personalBankCoins', 'sharedProfileBankCoins', 'bankCoinsUnclassified'];
const observedItems = z.object({ items: z.array(item).max(54), observedSlots: z.number().int().min(0).max(54), completeStorage: z.literal(false), greenhouseNextStageSecondsAtObservation: z.number().int().min(0).max(10 * 365 * 86400).optional() });
const labels = ["Level", "Tier", "Experience", "HotM Experience", "HotF Experience", "Mithril Powder", "Gemstone Powder", "Glacite Powder", "Tokens of the Mountain", "Tokens of the Forest", "Mining Speed", "Mining Fortune", "Foraging Fortune", "Hunting Fortune", "Charm Chance", "Pristine", "Breaking Power", "Magical Power", "Bonus Commissions Remaining", "Daily Bonus Commissions", "Charges", "Remaining Charges", "Copper", "Bits", "Motes", "Whispers", "Chocolate", "Progress", "Cost", "Next Level Cost", "Powder Cost", "Fuel", "Organic Matter", "Item Tier", "Collection", "Health", "Defense", "Strength", "Crit Damage", "Crit Chance", "Intelligence", "Speed", "Sea Creature Chance", "Fishing Speed", "Purse", "Piggy", "Bank", "Mana", "Farming Fortune", "Mining Wisdom", "Foraging Wisdom", "Combat Wisdom", "Fishing Wisdom", "Enchanting Wisdom", "Alchemy Wisdom", "Taming Wisdom", "Hunting Wisdom", "Ability Damage", "Bonus Attack Speed", "True Defense", "Vitality", "Mending", "Sweep", "Pressure Resistance", "Fossil Dust", "Soulflow", "Farming", "Mining", "Foraging", "Combat", "Fishing", "Hunting", "Enchanting", "Alchemy", "Taming", "Carpentry", "Runecrafting", "Social"];
const facts = z.object({ facts: z.array(z.object({ slot: z.number().int().min(0).max(53), displayName: display, fields: z.array(z.object({ label: z.enum(labels), value: z.number().finite().min(-1e15).max(1e15), maximum: z.number().finite().min(0).max(1e15).nullable(), unit: z.enum(['XP', 'coins', 'Powder', 'Whispers', 'Bits', 'Motes', '%']).nullable() })).max(64) })).max(54), observedSlots: z.number().int().min(0).max(54), completeSystem: z.literal(false), items: z.array(item).max(54).optional() });
const menuSections = ['mining', 'foraging', 'hunting', 'dailies', 'stats', 'pets', 'collections', 'accessories', 'quests', 'museum', 'orders', 'travel', 'fishing', 'bestiary', 'dungeons', 'slayers', 'events'];
const sections = SECTIONS;
export class ClientObservations {
    path;
    now;
    constructor(path, now = Date.now) {
        this.path = path;
        this.now = now;
    }
    async read() {
        if (!this.path)
            return null;
        let file;
        try {
            file = await open(this.path, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0));
            const stat = await file.stat();
            if (!stat.isFile() || stat.size > 1024 * 1024 || process.platform !== 'win32' && (stat.mode & 0o077) !== 0)
                return null;
            return snapshot.parse(JSON.parse(await file.readFile('utf8')));
        }
        catch {
            return null;
        }
        finally {
            await file?.close();
        }
    }
    async query(player, requestedProfile, section, options = {}) {
        if (!/^(?:[A-Za-z0-9_]{1,16}|[a-fA-F0-9-]{32,36})$/.test(player))
            throw new AppError('INVALID_PLAYER', 'Provide a Minecraft username or UUID.');
        if (requestedProfile !== undefined && !/^[A-Za-z][A-Za-z ]{0,31}$/.test(requestedProfile))
            throw new AppError('INVALID_PROFILE', 'Provide the exact profile name visible in the game.');
        if (section !== undefined && !sections.includes(section))
            throw new AppError('INVALID_SECTION', 'Choose a supported factual profile section.');
        const maxAge = options.maxAgeSeconds ?? 10, offset = options.offset ?? 0, limit = options.limit ?? 50;
        if (!Number.isInteger(maxAge) || maxAge < 2 || maxAge > 300 || !Number.isInteger(offset) || offset < 0 || offset > 50000 || !Number.isInteger(limit) || limit < 1 || limit > 100)
            throw new AppError('INVALID_LIVE_OPTIONS', 'Use maximum age 2–300 seconds and bounded pagination.');
        const unavailable = (reasonCode) => ({ source: 'client_observation', status: 'unavailable', reasonCode, observations: {}, apiRequests: 0, mode: options.live ? 'live' : 'latest_observations', warnings: ['Client observations are distinct from Hypixel API data. Missing observations do not mean zero or empty storage.'] });
        const raw = await this.read();
        if (!raw)
            return unavailable(this.path ? 'missing_or_invalid_private_snapshot' : 'collector_not_configured');
        const now = this.now();
        if (raw.exportedAt > now + 5000 || now - raw.exportedAt > 86400_000)
            return unavailable('snapshot_expired_or_future');
        const uuid = player.toLowerCase().replaceAll('-', '');
        let matches = raw.profiles.filter(p => p.uuid === uuid || p.playerName.toLowerCase() === player.toLowerCase());
        if (requestedProfile)
            matches = matches.filter(p => p.profileName.toLowerCase() === requestedProfile.toLowerCase());
        else if (matches.length > 1)
            matches = matches.filter(p => `${p.uuid}:${p.profileName.toLowerCase()}` === raw.activeProfile);
        if (matches.length > 1)
            throw new AppError('PROFILE_REQUIRED', 'Choose an exact profile name from your collected game data.');
        if (matches.length !== 1)
            return unavailable('no_matching_player_and_profile');
        const p = matches[0], out = {}, unavailableObservations = {}, pagination = {};
        const connected = raw.activeProfile === `${p.uuid}:${p.profileName.toLowerCase()}` && now - raw.exportedAt <= maxAge * 1000;
        if (options.live && !connected)
            return { ...unavailable('collector_not_connected_or_export_too_old'), mode: 'live', apiRequests: 0, exportedAt: new Date(raw.exportedAt).toISOString() };
        for (const [key, record] of Object.entries(p.observations)) {
            const age = now - record.observedAt;
            if (age < 0 || age >= 86400_000)
                continue;
            let group, parsed;
            if (key === 'cookie' || key === 'godPotion') {
                group = 'stats';
                const result = buff.safeParse(record.values);
                if (result.success)
                    parsed = result.data;
            }
            else if (bankFields.some(f => key === `bank:${f}`)) {
                group = 'currencies';
                const field = key.slice(5);
                const result = z.object({ [field]: coins }).safeParse(record.values);
                if (result.success)
                    parsed = result.data;
            }
            else if (key === 'huntaxe') {
                group = 'equipment';
                const result = z.object({ weapon: item }).safeParse(record.values);
                if (result.success)
                    parsed = result.data;
            }
            else if (key === 'inventory' || key === 'toolkit' || /^storage:(?:(?:Small |Medium |Large |Greater |Jumbo )?Backpack(?: \(Slot #\d{1,2}\))?|Ender Chest(?: \(\d+\/\d+\))?|Storage \(\d+\/\d+\))$/.test(key)) {
                group = 'equipment';
                const result = observedItems.safeParse(record.values);
                if (result.success)
                    parsed = result.data;
            }
            else if (key === 'minion' || key === 'garden') {
                group = key === 'minion' ? 'minions' : 'garden';
                const result = observedItems.safeParse(record.values);
                if (result.success)
                    parsed = result.data;
            }
            else if (key === 'liveState') {
                group = 'stats';
                const result = z.object({ selectedSlot: z.number().int().min(0).max(8), minecraftHealth: z.number().finite().nonnegative(), minecraftMaxHealth: z.number().finite().nonnegative(), minecraftExperienceLevel: z.number().int().nonnegative(), minecraftExperienceProgress: z.number().finite().min(0).max(1), dungeonHudObserved: z.boolean(), minecraftEffects: z.array(z.object({ id: z.string().regex(/^[a-z0-9_.:-]{1,96}$/), amplifier: z.number().int().min(0).max(255), remainingTicksAtObservation: z.number().int().min(-1).max(1e9), infinite: z.boolean(), ambient: z.boolean(), visible: z.boolean() })).max(64) }).safeParse(record.values);
                if (result.success)
                    parsed = result.data;
            }
            else if (key === 'sidebar' || key === 'actionBar') {
                group = 'stats';
                const result = facts.safeParse(record.values);
                if (result.success)
                    parsed = result.data;
            }
            else if (menuSections.includes(key) || key === 'tabStats') {
                group = key === 'tabStats' ? 'stats' : key;
                const result = facts.safeParse(record.values);
                if (result.success)
                    parsed = result.data;
            }
            if (!parsed || section && section !== 'api_fields' && section !== 'systems' && section !== group && !(section === 'storage' && group === 'equipment'))
                continue;
            const current = connected && age <= maxAge * 1000 && (raw.observedNow?.includes(key) ?? (key === 'inventory'));
            if (options.live && !current) {
                unavailableObservations[key] = { reasonCode: 'not_currently_observed_or_too_old', ageSeconds: Math.floor(age / 1000), readFrom: record.readFrom };
                continue;
            }
            const values = { ...parsed };
            for (const name of ['items', 'facts'])
                if (Array.isArray(values[name])) {
                    const all = values[name];
                    values[name] = all.slice(offset, offset + limit);
                    pagination[key + '.' + name] = { offset, limit, total: all.length, returned: values[name].length, hasMore: offset + limit < all.length };
                }
            if (Array.isArray(values.items))
                values.items = values.items.map(i => ({ ...i, name: i.displayName ?? null, location: key === 'inventory' ? 'inventory' : key, equippedState: key === 'inventory' ? Number(i.slot) >= 36 && Number(i.slot) <= 39 ? 'minecraft_armor_slot' : Number(i.slot) === 40 ? 'minecraft_offhand' : 'carried' : null, rarityUpgrade: i.rarity_upgrades ?? null, dungeonUpgradeLevel: i.upgrade_level ?? i.dungeon_item_level ?? null, upgrades: Object.fromEntries(Object.entries(i).filter(([k, v]) => typeof v === 'number' && !['slot', 'count'].includes(k))) }));
            if (values.weapon)
                values.weapon = { ...values.weapon, name: values.weapon.displayName ?? null, rarityUpgrade: values.weapon.rarity_upgrades ?? null, dungeonUpgradeLevel: values.weapon.upgrade_level ?? values.weapon.dungeon_item_level ?? null };
            out[key] = { source: 'client_observation', status: age > 300_000 ? 'stale' : 'available', observedAt: new Date(record.observedAt).toISOString(), ageSeconds: Math.floor(age / 1000), readFrom: record.readFrom, values, coverage: key === 'huntaxe' || key.startsWith('bank:') || group === 'stats' ? 'only_the_displayed_field' : key === 'inventory' ? 'only_current_player_inventory_slots' : 'partial_only_viewed_menu', currentStateVerified: current, scope: 'member' };
        }
        const mappedFields = [];
        for (const [key, record] of Object.entries(out)) {
            const row = record, values = row.values;
            for (const [field, value] of Object.entries(values)) {
                if (typeof value === 'number' || typeof value === 'boolean')
                    mappedFields.push({ path: 'client.' + key + '.' + field, value, status: 'known', scope: 'member', observedAt: row.observedAt, currentStateVerified: row.currentStateVerified, canonicalApiPath: key.startsWith('bank:') ? (field === 'personalBankCoins' ? 'member.profile.bank_account' : field === 'sharedProfileBankCoins' ? 'profile.banking.balance' : null) : null });
            }
        }
        return { changesSincePreviousObservations: (p.changes ?? []).filter(c => c.observedAt <= now && now - c.observedAt < 86400000), changeCoverage: { maximum: 64, source: 'passive_owner_observations', persistentHistory: false, removedItemsDoNotProveSale: true }, fields: mappedFields, source: 'client_observation', status: Object.keys(out).length ? 'partial' : 'unavailable', reasonCode: Object.keys(out).length ? 'owner_visible_observations' : 'no_valid_recent_observations', player: { uuid: p.uuid, name: p.playerName }, profile: { name: p.profileName }, mode: options.live ? 'live' : 'latest_observations', apiRequests: 0, maxAgeSeconds: maxAge, unavailableObservations, pagination, collectorCoverage: { evictedObservations: raw.evictedObservations ?? 0, exportIntervalMs: raw.exportIntervalMs ?? 5000, onlyReceivedVisibleState: true, completeProfile: false }, collectorVersion: raw.collectorVersion, exportedAt: new Date(raw.exportedAt).toISOString(), connectedAtLastExport: raw.activeProfile === `${p.uuid}:${p.profileName.toLowerCase()}` && now - raw.exportedAt < 45000, observations: out, warnings: ['Owner-only client observations; they are not Hypixel API responses or independently verified values.', 'Timers are remaining time when observed, not a live countdown. Gameplay can pause effects.', 'Current inventory, owner state, selected item attributes and displayed HUD/tab/sidebar facts are passive. Server-unsent fields still require a viewed menu or remain unknown. Unviewed storage and minions remain unknown. Data expires after 24 hours; each key retains only its latest observation.'] };
    }
}
