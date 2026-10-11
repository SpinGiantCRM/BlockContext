import { readFileSync } from 'node:fs';

// Pinned, MIT-licensed game data. There is no remote code or background updater.
const accessories = JSON.parse(readFileSync(new URL('../../fixtures/domain/misc.json', import.meta.url), 'utf8'));
const garden = JSON.parse(readFileSync(new URL('../../fixtures/domain/garden.json', import.meta.url), 'utf8'));
const known = JSON.parse(readFileSync(new URL('../../fixtures/domain/known-accessories.json', import.meta.url), 'utf8'));
export const KNOWN_ACCESSORIES = new Set<string>(known.ids);
export const DOMAIN_REVISION: string = accessories.commit;
export const UPGRADE_RELATIONS = accessories.data.talisman_upgrades as Record<string, string[]>;
export const IGNORED_ACCESSORIES = new Set<string>(accessories.data.ignored_talisman);
export const GARDEN_EXPERIENCE = garden.data.garden_exp as number[];
export const CROP_MILESTONES = garden.data.crop_milestones as Record<string, number[]>;
export const MAGICAL_POWER: Record<string, number> = { COMMON: 3, UNCOMMON: 5, RARE: 8, EPIC: 12, LEGENDARY: 16, MYTHIC: 22, SPECIAL: 3, 'VERY SPECIAL': 5 };
