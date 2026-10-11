export type JsonObject = Record<string, unknown>;
export const SECTIONS = ['stats', 'skills', 'currencies', 'slayers', 'dungeons', 'pets', 'collections', 'minions', 'equipment', 'accessories', 'garden'] as const;
export type SectionName = typeof SECTIONS[number];
export interface Section {
  status: 'available' | 'partial' | 'unavailable';
  data: JsonObject;
  warnings: string[];
}
export interface Identity { uuid: string; name: string | null }
export interface NormalizedProfile {
  id: string;
  name: string;
  selected: boolean;
  gameMode: string | null;
  sections: Record<SectionName, Section>;
}
export interface ProfileSet { profiles: NormalizedProfile[]; retrievedAt: number }
export interface Upstream {
  readonly source: 'hypixel' | 'synthetic_demo';
  resolve(player: string): Promise<Identity>;
  profiles(uuid: string): Promise<JsonObject[]>;
  skillCatalog(): Promise<JsonObject>;
  itemCatalog(): Promise<JsonObject>;
  playerMetadata(uuid: string): Promise<JsonObject>;
  garden(profile: string): Promise<JsonObject | null>;
  bazaar(): Promise<JsonObject>;
  auctionPage(page: number, signal?: AbortSignal): Promise<JsonObject>;
}
export class AppError extends Error {
  constructor(public readonly code: string, message: string, public readonly transient = false, public readonly retryAfterSeconds?: number) { super(message); }
}
export function object(value: unknown): JsonObject {
  return value !== null && typeof value === 'object' && !Array.isArray(value) ? value as JsonObject : {};
}
export function at(value: unknown, ...path: string[]): unknown {
  for (const key of path) value = object(value)[key];
  return value;
}
export function number(value: unknown): number | null { return typeof value === 'number' && Number.isFinite(value) ? value : null; }
export function text(value: unknown, max = 96): string | null {
  return typeof value === 'string' ? value.replace(/§[0-9a-fk-or]/gi, '').replace(/[\u0000-\u001f\u007f]/g, '').slice(0, max) : null;
}
export function numericMap(value: unknown, limit = 2048): Record<string, number> {
  return Object.fromEntries(Object.entries(object(value)).slice(0, limit).filter(([k, v]) => /^[a-zA-Z0-9_:.-]{1,96}$/.test(k) && number(v) !== null)) as Record<string, number>;
}
