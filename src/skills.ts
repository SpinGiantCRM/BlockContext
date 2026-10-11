import { at, number, object, text, type JsonObject, type Section } from './types.js';

function cap(skill: string, member: JsonObject, player: JsonObject, maximum: number | null): { value: number | null; minimum: number | null; source: string } {
  const record = (v: unknown) => v !== null && typeof v === 'object' && !Array.isArray(v);
  if (skill === 'farming') {
    const perks = at(member,'jacobs_contest','perks');
    const bonus = at(perks,'farming_level_cap');
    if (record(perks) && bonus === undefined) return { value: Math.min(50, maximum ?? 50), minimum: Math.min(50, maximum ?? 50), source: 'Jacobs Contest perks: no cap upgrade recorded.' };
    if (Number.isInteger(bonus) && Number(bonus) >= 0 && Number(bonus) <= 10) return { value: Math.min(50+Number(bonus), maximum ?? 60), minimum: 50, source: 'Jacobs Contest farming_level_cap upgrades.' };
    return { value: null, minimum: 50, source: 'Farming cap upgrades are unavailable or invalid.' };
  }
  if (skill === 'taming') {
    const pets = member.pets_data, care = at(pets,'pet_care'), sacrificed = at(care,'pet_types_sacrificed');
    if (record(pets) && (care === undefined || record(care)) && sacrificed === undefined) return { value: Math.min(50, maximum ?? 50), minimum: Math.min(50, maximum ?? 50), source: 'No George sacrifices recorded in pets_data.' };
    if (Array.isArray(sacrificed) && sacrificed.length <= 64 && sacrificed.every(v=>typeof v === 'string' && /^[A-Z_]{1,64}$/.test(v)) && new Set(sacrificed).size <= 10) return { value: Math.min(50+new Set(sacrificed).size, maximum ?? 60), minimum: 50, source: 'Unique pet types sacrificed to George.' };
    return { value: null, minimum: 50, source: 'George sacrifice data is unavailable or invalid.' };
  }
  if (skill === 'runecrafting') {
    const ranks = [player.rank, player.packageRank, player.newPackageRank, player.monthlyPackageRank].filter(v=>typeof v === 'string');
    const paid = ranks.some(r=>['VIP','VIP_PLUS','MVP','MVP_PLUS','SUPERSTAR','YOUTUBER','ADMIN','MODERATOR','GAME_MASTER','HELPER'].includes(String(r)));
    const known = ranks.length > 0 && ranks.every(r=>['NONE','NORMAL','VIP','VIP_PLUS','MVP','MVP_PLUS','SUPERSTAR','YOUTUBER','ADMIN','MODERATOR','GAME_MASTER','HELPER'].includes(String(r)));
    return { value: paid ? Math.min(25,maximum ?? 25) : known ? 3 : null, minimum: 3, source: paid || known ? 'Hypixel player rank metadata.' : 'Rank metadata unavailable or unrecognized.' };
  }
  if (skill === 'foraging') return { value: null, minimum: 50, source: 'Galatea/Torrhus unlock gates are not fully exposed by the profile API; catalog maximum is not the player cap.' };
  return { value: maximum, minimum: maximum, source: 'Current Hypixel skill resource table.' };
}

export function skillSection(member: JsonObject, catalog: JsonObject, player: JsonObject = {}): Section {
  const legacy = Object.fromEntries(Object.entries(member).filter(([k]) => k.startsWith('experience_skill_')).map(([k, v]) => [`SKILL_${k.slice(17).toUpperCase()}`, v]));
  const values = { ...legacy, ...object(at(member, 'player_data', 'experience')) };
  const skills: JsonObject = {};
  for (const [key, value] of Object.entries(values).slice(0, 64)) {
    if (!/^SKILL_[A-Z_]{1,32}$/.test(key)) continue;
    const xp = number(value); if (xp === null || xp < 0) continue;
    const definition = object(at(catalog, 'skills', key.slice(6)));
    const levels = Array.isArray(definition.levels) ? definition.levels.map(object).sort((a,b) => Number(a.level)-Number(b.level)) : [];
    const valid = levels.length > 0 && levels.length <= 256 && levels.every((l,i) => Number.isInteger(l.level) && Number(l.level) === i + 1 && number(l.totalExpRequired) !== null && Number(l.totalExpRequired) > (i ? Number(levels[i-1]!.totalExpRequired) : 0));
    let xpLevel: number | null = valid ? 0 : null, base: number | null = valid ? 0 : null, next: number | null = null;
    if (valid) for (const level of levels) {
      if (xp >= Number(level.totalExpRequired)) { xpLevel = Number(level.level); base = Number(level.totalExpRequired); }
      else { next = Number(level.totalExpRequired); break; }
    }
    const maximum = number(definition.maxLevel);
    const catalogMaximumLevel = valid && maximum !== null && Number.isInteger(maximum) && maximum === Number(levels.at(-1)!.level) ? maximum : null;
    const progressToNextLevel = base !== null && next !== null ? (xp - base) / (next - base) : null;
    const unlocks = next === null ? undefined : levels.find(l => Number(l.totalExpRequired) === next)?.unlocks;
    const capInfo = cap(key.slice(6).toLowerCase(), member, player, catalogMaximumLevel);
    const effectiveLevel = xpLevel === null ? null : capInfo.value !== null ? Math.min(xpLevel,capInfo.value) : capInfo.minimum !== null && xpLevel <= capInfo.minimum ? xpLevel : null;
    const uncertainProgress = capInfo.value === null && xpLevel !== null && capInfo.minimum !== null && xpLevel >= capInfo.minimum;
    const capped = effectiveLevel !== null && capInfo.value !== null && effectiveLevel >= capInfo.value;
    skills[key.slice(6).toLowerCase()] = {
      experience: xp, xpLevel, xpLevelWithProgress: xpLevel === null ? null : xpLevel + (progressToNextLevel ?? 0),
      experienceToNextLevel: next === null ? null : next - xp,
      experienceIntoCurrentLevel: base === null ? null : xp - base,
      progressToNextLevel, catalogMaximumLevel,
      catalogMaximumReached: catalogMaximumLevel === null ? null : xpLevel === catalogMaximumLevel,
      playerLevelCap: capInfo.value, playerLevelCapMinimum: capInfo.minimum, playerLevelCapMaximum: catalogMaximumLevel, capSource: capInfo.source,
      effectiveLevel, effectiveLevelWithProgress: effectiveLevel === null || uncertainProgress ? null : capped ? effectiveLevel : effectiveLevel+(progressToNextLevel ?? 0),
      atPlayerCap: capInfo.value === null || effectiveLevel === null ? null : capped,
      effectiveExperienceToNextLevel: effectiveLevel === null || capped || uncertainProgress ? null : next === null ? null : next-xp,
      nextLevelUnlocks: Array.isArray(unlocks) ? unlocks.slice(0, 24).map(v => text(v, 256)).filter(v => v !== null) : null
    };
  }
  const warnings = ['XP-derived levels and next-level unlocks are reference data. Use effectiveLevel and playerLevelCap for capped progression; unknown cap gates remain explicit. Social XP is for this member, not a co-op aggregate.'];
  if (Object.values(skills).some(v => at(v, 'xpLevel') === null)) warnings.push('Some skill tables are unavailable or invalid; their XP is provided without level conversion.');
  return { status: Object.keys(skills).length ? 'partial' : 'unavailable', data: { skills, catalogLastUpdated: number(catalog.lastUpdated) }, warnings };
}
