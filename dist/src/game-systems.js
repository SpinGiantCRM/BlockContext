import { whisperBalances } from './whispers.js';
import { treeNodes, definitions } from './system-definitions.js';
import { readFileSync } from 'node:fs';
import { at, number, object, text } from './types.js';
const rules = JSON.parse(readFileSync(new URL('../../fixtures/domain/skill-trees.json', import.meta.url), 'utf8'));
export const EXTRA_SECTIONS = ['mining', 'foraging', 'hunting', 'dailies', 'quests', 'rift', 'crimson_isle', 'chocolate_factory', 'systems'];
const safeKey = /^[A-Za-z0-9_: .-]{1,96}$/;
// Game evidence only. Never project identities, messages, login/session fields or encoded inventories.
const denied = /^(?:uuid|player_uuid|player_id|display_name|teammates|participants|members|session|sessions|last_login|last_logout|first_join|chat|messages|data|email|token|access_token|started_by|claimed_by|who_started)$/i;
export function gameEvidence(value) {
    let budget = 2048, truncated = false;
    const visit = (v, depth) => {
        if (--budget < 0 || depth > 10) {
            truncated = true;
            return null;
        }
        if (v === null || typeof v === 'boolean')
            return v;
        if (typeof v === 'number')
            return Number.isFinite(v) ? v : null;
        if (typeof v === 'string') {
            if (v.length > 128)
                truncated = true;
            return /^[a-f0-9]{32}$|^[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/i.test(v) ? null : text(v, 128);
        }
        if (Array.isArray(v)) {
            if (v.length > 2048)
                truncated = true;
            return v.slice(0, 2048).map(x => visit(x, depth + 1));
        }
        if (typeof v !== 'object')
            return null;
        const entries = Object.entries(object(v));
        if (entries.length > 2048)
            truncated = true;
        return Object.fromEntries(entries.slice(0, 2048).filter(([k]) => safeKey.test(k) && !denied.test(k) && !(/^[a-f0-9]{32}$|^[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/i.test(k))).map(([k, x]) => [k, visit(x, depth + 1)]));
    };
    return { value: visit(value, 0), truncated };
}
export function field(value, source, unit = null, scope = 'member') {
    const known = value !== undefined && value !== null;
    return { status: known ? 'known' : 'unknown', value: known ? value : null, unit, scope, source, reasonCode: known ? null : 'not_exposed_or_not_verified', observedAt: 'see_response_metadata.retrievedAt', gameUpdatedAt: null };
}
function nonnegative(value) { const n = number(value); return n !== null && n >= 0 ? n : null; }
function numericFields(v) { return Object.fromEntries(Object.entries(object(v)).filter(([k, x]) => safeKey.test(k) && number(x) !== null)); }
function evidence(v, path) {
    const p = gameEvidence(v), reported = field(p.value, path, null, path.startsWith('profile.') ? 'shared_profile' : 'member');
    return { ...reported, status: p.value !== null && p.truncated ? 'partial' : reported.status, truncated: p.truncated, reasonCode: p.truncated ? 'evidence_projection_truncated' : reported.reasonCode, interpretation: 'Reported game fields; missing values are not zero, and undocumented counters and timestamps are not inferred to be current completions or unlocks.' };
}
function tree(m, kind) {
    const modern = at(m, 'skill_tree', 'experience', kind), legacy = at(m, kind === 'mining' ? 'mining_core' : 'foraging_core', 'experience');
    const xp = nonnegative(modern ?? legacy), curve = rules.cumulativeExperience[kind], nodesRoot = at(m, 'skill_tree', 'nodes') ?? at(m, kind === 'mining' ? 'mining_core' : 'foraging_core', 'nodes');
    let level = null, next = null, base = null;
    if (xp !== null) {
        level = 0;
        for (let i = 0; i < curve.length; i++) {
            if (xp >= curve[i]) {
                level = i + 1;
                base = curve[i];
            }
            else {
                next = curve[i];
                break;
            }
        }
    }
    const slots = Object.entries(object(nodesRoot)).filter(([k]) => k === kind || new RegExp('^' + kind + '_[2-5]$').test(k));
    const presets = slots.length ? slots : [[kind, object(nodesRoot)]];
    const nodes = treeNodes(kind, presets, level), coreRules = rules.treeTokens[kind], core = nonnegative(at(m, 'skill_tree', 'nodes', kind, coreRules.coreId)), spent = nonnegative(at(m, 'skill_tree', 'tokens_spent', kind === 'mining' ? 'mountain' : 'forest'));
    const totalTokens = level !== null && core !== null && core <= coreRules.maxCoreLevel ? coreRules.tier.slice(0, level).reduce((a, b) => a + b, 0) + coreRules.core.slice(0, core).reduce((a, b) => a + b, 0) : null;
    return { experience: field(xp, modern !== undefined ? 'member.skill_tree.experience.' + kind : 'member.' + (kind === 'mining' ? 'mining_core' : 'foraging_core') + '.experience', 'XP'), level: field(level, 'calculated_from_reported_tree_experience', 'tier'), experienceIntoTier: field(xp !== null && base !== null ? xp - base : null, 'calculated', 'XP'), experienceToNextTier: field(xp !== null && next !== null ? next - xp : null, 'calculated', 'XP'), cap: curve.length, atCap: xp !== null ? level === curve.length : null, nodes, nodesCoverage: { returned: nodes.length, presetEntriesMayBeLimited: true, allLockedNodesKnown: false, definitionsAvailable: true, pinnedReferenceMayBeOlderThanGame: true }, selectedPreset: field(at(m, 'skill_tree', 'selected_skill_tree_slot', kind), 'member.skill_tree.selected_skill_tree_slot.' + kind), selectedAbility: field(at(m, 'skill_tree', 'selected_ability', kind), 'member.skill_tree.selected_ability.' + kind), tokensSpent: field(nonnegative(at(m, 'skill_tree', 'tokens_spent', kind === 'mining' ? 'mountain' : 'forest')), 'member.skill_tree.tokens_spent', 'tokens'), tokensAvailable: field(totalTokens !== null && spent !== null && totalTokens >= spent ? totalTokens - spent : null, 'calculated_from_reported_tier_core_spent_and_pinned_rewards', 'tokens'), tokensEarned: field(totalTokens, 'calculated_pinned_tree_rewards', 'tokens'), core: { id: coreRules.coreId, level: field(core, 'member.skill_tree.nodes.' + kind + '.' + coreRules.coreId), maximumLevel: coreRules.maxCoreLevel }, resetEvidence: evidence(at(m, 'skill_tree', 'last_reset'), 'member.skill_tree.last_reset'), freeResetReportedFlag: field(at(m, 'skill_tree', 'refund_ability_free'), 'member.skill_tree.refund_ability_free'), rules: { source: rules.source, revision: rules.commit, reviewedAt: rules.reviewedAt, kind: 'pinned_community_cumulative_experience', liveGameRulesVerified: false } };
}
function section(data, available, warnings = []) { return { status: available ? 'partial' : 'unavailable', data, warnings: ['Missing or undocumented fields stay unknown. Reported API evidence is not a complete model of this system.', ...warnings] }; }
function activities(m) {
    const definitions = [['hotm_bonus_commissions', 'HotM daily bonus commissions', null], ['experimentation', 'Experimentation Table', m.experimentation], ['foraging_gifts', 'Foraging gifts', m.foraging_core], ['forge', 'Forge jobs', m.forge], ['dungeons_daily', 'Dungeon daily runs', at(m, 'dungeons', 'daily_runs')], ['garden', 'Garden visitors, pests and composter', m.garden_player_data], ['chocolate_factory', 'Chocolate Factory', at(m, 'events', 'easter')], ['crimson_daily', 'Crimson Isle daily activities', m.nether_island_player_data], ['community_upgrades', 'Community upgrades', m.community_upgrades]];
    return definitions.map(([id, name, value]) => ({ activityId: id, name, status: value === undefined || value === null ? 'unknown' : 'partial', eligible: null, completedToday: null, remainingCharges: null, nextReset: null, reward: null, certainty: 'No completion is inferred from a missing field or an undocumented timestamp.', evidence: evidence(value, (id === 'community_upgrades' ? 'profile.' : 'member.') + id) }));
}
export function gameSystems(m, raw) {
    const miningTree = tree(m, 'mining'), foragingTree = tree(m, 'foraging');
    const powders = ['mithril', 'gemstone', 'glacite'].map(type => ({ type, reportedPowder: field(nonnegative(at(m, 'mining_core', 'powder_' + type)), 'member.mining_core.powder_' + type, 'powder'), reportedTotal: field(nonnegative(at(m, 'mining_core', 'powder_' + type + '_total')), 'member.mining_core.powder_' + type + '_total', 'powder'), spent: field(nonnegative(at(m, 'mining_core', 'powder_spent_' + type)), 'member.mining_core.powder_spent_' + type, 'powder'), available: field(nonnegative(at(m, 'mining_core', 'powder_' + type)), 'member.mining_core.powder_' + type, 'powder'), counterRules: { revision: rules.counterRulesRevision, source: 'SkyCrypt mining counter implementation', liveGameVerified: false } }));
    const forgeRaw = at(m, 'forge', 'forge_processes'), forgeJobs = Object.entries(object(forgeRaw)).slice(0, 16).flatMap(([forge, value]) => Object.entries(object(value)).slice(0, 64).map(([slot, v]) => ({ forge, slot, reported: evidence(v, 'member.forge.forge_processes.' + forge + '.' + slot), completionTime: null, readyToClaim: null, reasonCode: 'recipe_duration_and_modifiers_not_verified' })));
    const objectives = Object.entries(object(m.objectives)).slice(0, 4096).filter(([id, v]) => safeKey.test(id) && v !== null && typeof v === 'object' && !Array.isArray(v)).map(([id, v]) => ({ id, status: text(at(v, 'status')), progress: nonnegative(at(v, 'progress')), completedAt: nonnegative(at(v, 'completed_at')) === 0 ? null : nonnegative(at(v, 'completed_at')), evidence: evidence(v, 'member.objectives.' + id) }));
    const owned = at(m, 'shards', 'owned'), shards = Array.isArray(owned) ? owned.slice(0, 2048).map(v => ({ id: text(at(v, 'type')), amount: nonnegative(at(v, 'amount_owned')), lastCapturedAt: nonnegative(at(v, 'captured')) })) : null;
    const parts = {
        mining: section({ heartOfTheMountain: miningTree, powders, coreOfTheMountain: miningTree.core, commissions: { active: null, dailyBonusesRemaining: null, reasonCode: 'not_exposed_in_verified_profile_fields' }, forgeJobs, forgeCoverage: field(forgeRaw, 'member.forge.forge_processes'), crystals: evidence(at(m, 'mining_core', 'crystals'), 'member.mining_core.crystals'), nucleus: evidence(at(m, 'mining_core', 'nucleus'), 'member.mining_core.nucleus'), goldCollection: field(nonnegative(at(m, 'collection', 'GOLD_INGOT')), 'member.collection.GOLD_INGOT', 'items'), coreEvidence: evidence(m.mining_core, 'member.mining_core') }, m.skill_tree !== undefined || m.mining_core !== undefined),
        foraging: section({ heartOfTheForest: foragingTree, whispers: whisperBalances(m), centerOfTheForest: foragingTree.core, coreEvidence: evidence(m.foraging_core, 'member.foraging_core'), foragingEvidence: evidence(m.foraging, 'member.foraging') }, m.foraging_core !== undefined || m.foraging !== undefined),
        hunting: section({ shards, shardCoverage: field(owned, 'member.shards.owned'), attributes: evidence(m.attributes, 'member.attributes'), traps: evidence(at(m, 'shards', 'traps'), 'member.shards.traps'), fusion: evidence(at(m, 'shards', 'fused'), 'member.shards.fused'), toolkitEvidence: evidence(at(m, 'foraging', 'hunting_toolkit'), 'member.foraging.hunting_toolkit'), shardDefinitions: at(definitions, 'shards', 'attributes'), attributeLevelThresholds: at(definitions, 'shards', 'attribute_levelling'), effectiveStats: null, attributeLevels: null, reasonCode: 'stack_counts_are_not_verified_effective_attribute_levels' }, m.shards !== undefined || m.attributes !== undefined),
        dailies: section({ activities: activities({ ...m, community_upgrades: raw.community_upgrades }), resetTimezone: 'UTC day boundaries and per-activity resets remain unverified; no automatic completedToday inference.', liveModifiersUnknown: true }, true),
        quests: section({ objectives, objectiveCoverage: { expected: Object.keys(object(m.objectives)).length, returned: objectives.length, truncated: Object.keys(object(m.objectives)).length > 4096 }, quests: evidence(m.quests, 'member.quests'), unlockEvidence: evidence({ visitedZones: at(m, 'player_data', 'visited_zones'), visitedModes: at(m, 'player_data', 'visited_modes'), unlockedCollectionTiers: at(m, 'player_data', 'unlocked_coll_tiers') }, 'member.player_data'), levelingEvidence: evidence(m.leveling, 'member.leveling'), communityUpgrades: evidence(raw.community_upgrades, 'profile.community_upgrades') }, m.objectives !== undefined || m.quests !== undefined),
        rift: section({ progress: evidence(m.rift, 'member.rift') }, m.rift !== undefined),
        crimson_isle: section({ progress: evidence(m.nether_island_player_data, 'member.nether_island_player_data') }, m.nether_island_player_data !== undefined),
        chocolate_factory: section({ progress: evidence(at(m, 'events', 'easter'), 'member.events.easter'), chocolate: field(nonnegative(at(m, 'events', 'easter', 'chocolate')), 'member.events.easter.chocolate', 'chocolate'), productionPerSecond: null, reasonCode: 'live_production_formula_not_verified' }, at(m, 'events', 'easter') !== undefined),
        systems: section({ registry: [], bestiary: evidence(m.bestiary, 'member.bestiary'), reportedCombatRecords: field(numericFields(m.player_stats), 'member.player_stats'), reportedSystemFields: Object.keys(m).filter(k => safeKey.test(k) && !denied.test(k)), unsupportedViews: ['persistent_user_facts', 'goal_tracking', 'recommendations', 'money_method_comparison', 'continuous_API_session_history'] }, true)
    };
    parts.systems.data.registry = Object.entries(parts).filter(([id]) => id !== 'systems').map(([id, s]) => ({ id, status: s.status, source: 'profile_API', gameScope: 'member_unless_a_field_says_profile', fullyImplemented: false }));
    return parts;
}
