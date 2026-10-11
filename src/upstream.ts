import { Diagnostics } from './diagnostics.js';
import { AppError, at, number, object, text, type Identity, type JsonObject, type Upstream } from './types.js';

export const UUID = /^[0-9a-f]{32}$/i;
export function uuidValue(value: string): string | null {
  const flat = value.replaceAll('-', '');
  return UUID.test(flat) && (/^[0-9a-f]{32}$/i.test(value) || /^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(value)) ? flat.toLowerCase() : null;
}
export function validatePlayer(player: string): string {
  if (uuidValue(player) || /^[a-zA-Z0-9_]{1,16}$/.test(player)) return player;
  throw new AppError('INVALID_PLAYER', 'Provide a Minecraft username or UUID.');
}

async function limitedJson(response: Response, signal: AbortSignal, maxBytes = 12 * 1024 * 1024): Promise<JsonObject> {
  if (!response.body) throw new AppError('UPSTREAM_INVALID', 'The upstream response was empty.', true);
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = []; let size = 0;
  try {
    while (true) {
      const chunk = await reader.read(); if (chunk.done) break;
      size += chunk.value.byteLength;
      if (size > maxBytes) { await reader.cancel(); throw new AppError('UPSTREAM_TOO_LARGE', 'The upstream response exceeded the size limit.'); }
      chunks.push(chunk.value);
    }
    return object(JSON.parse(Buffer.concat(chunks).toString('utf8')));
  } catch (error) {
    if (error instanceof AppError) throw error;
    if (signal.aborted) throw new AppError('UPSTREAM_TIMEOUT', 'The upstream request exceeded its time limit.', true);
    if (!(error instanceof SyntaxError)) throw new AppError('UPSTREAM_UNAVAILABLE', 'The upstream response could not be read.', true);
    throw new AppError('UPSTREAM_INVALID', 'The upstream response was not valid JSON.', true);
  }
}

export class HypixelUpstream implements Upstream {
  readonly source = 'hypixel' as const;
  private calls: number[] = [];
  private blockedUntil = 0;
  private remaining: number | null = null;
  private resetAt = 0;
  private queue: Promise<unknown> = Promise.resolve();
  private readonly localLimit: number;
  private readonly diagnostics:Diagnostics;
  private readonly keyExpiresAt: number | undefined;
  constructor(private readonly key: string, private readonly request: typeof fetch = fetch, private readonly now = Date.now, options: { requestsPerFiveMinutes?: number; keyExpiresAt?: string; diagnostics?:Diagnostics } = {}) {
    this.diagnostics=options.diagnostics ?? new Diagnostics();
    this.localLimit = options.requestsPerFiveMinutes ?? 30;
    if (!Number.isInteger(this.localLimit) || this.localLimit < 1 || this.localLimit > 10000) throw new AppError('INVALID_CONFIG', 'The five-minute request budget must be an integer from 1 to 10000.');
    this.keyExpiresAt = options.keyExpiresAt ? Date.parse(options.keyExpiresAt) : undefined;
    if (options.keyExpiresAt && (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/.test(options.keyExpiresAt) || !Number.isFinite(this.keyExpiresAt))) throw new AppError('INVALID_CONFIG', 'The key expiry must be an ISO date with an explicit timezone.');
  }
  private async get(url: string, authenticated: boolean, signal?: AbortSignal): Promise<JsonObject> {
    const path=new URL(url).pathname;
    const endpoint=path.startsWith('/users/profiles/minecraft/')?'identity':({'/v2/skyblock/profiles':'profiles','/v2/resources/skyblock/skills':'skills','/v2/resources/skyblock/items':'items','/v2/player':'rank','/v2/skyblock/garden':'garden','/v2/skyblock/bazaar':'bazaar','/v2/skyblock/auctions':'auctions'} as Record<string,string>)[path] ?? 'unknown';
    const send = async () => {
      const started=performance.now(); let status:number|null=null;
      try {
      if (authenticated) {
        if (!this.key) throw new AppError('HYPIXEL_KEY_REQUIRED', 'Set HYPIXEL_API_KEY privately in the local .env file to enable live mode.');
        const now = this.now(); this.calls = this.calls.filter(t => now - t < 300_000);
        if (this.keyExpiresAt !== undefined && now >= this.keyExpiresAt) throw new AppError('HYPIXEL_KEY_EXPIRED', 'The configured Hypixel development key has expired. Replace it privately in the local .env file.');
        if (now >= this.resetAt) this.remaining = null;
        if (this.blockedUntil > now || this.calls.length >= this.localLimit || (this.remaining !== null && this.remaining <= 2)) {
          const retry = Math.max(this.blockedUntil, this.calls.length >= this.localLimit ? this.calls[0]! + 300_000 : now, this.remaining !== null && this.remaining <= 2 ? this.resetAt : now);
          throw new AppError('RATE_LIMITED', 'Hypixel request budget is reserved or temporarily exhausted. Try again later.', true, Math.max(1, Math.ceil((retry - now) / 1000)));
        }
        this.calls.push(now);
      }
      let response: Response;
      const requestSignal = signal ? AbortSignal.any([signal,AbortSignal.timeout(10_000)]) : AbortSignal.timeout(10_000);
      try {
        response = await this.request(url, { headers: authenticated ? { 'API-Key': this.key } : {}, signal: requestSignal, redirect: 'error' });
      } catch { throw requestSignal.aborted ? new AppError('UPSTREAM_TIMEOUT', 'The upstream request exceeded its time limit.', true) : new AppError('UPSTREAM_UNAVAILABLE', 'The upstream service could not be reached.', true); }
      status=response.status;
      if (authenticated) {
        const rem = response.headers.get('ratelimit-remaining'), reset = response.headers.get('ratelimit-reset');
        if (rem !== null && /^\d+$/.test(rem)) this.remaining = Number(rem);
        if (reset !== null && /^\d+$/.test(reset)) this.resetAt = this.now() + Math.min(Number(reset), 3600) * 1000;
        else if (this.remaining !== null) this.resetAt = this.now() + 300_000;
      }
      if (response.status === 429) {
        const header = response.headers.get('retry-after');
        const retry = header && /^\d+$/.test(header) ? Math.min(Number(header), 3600) : 60;
        if (authenticated) this.blockedUntil = this.now() + retry * 1000;
        throw new AppError('RATE_LIMITED', 'The upstream service is throttling requests.', true, retry);
      }
      if (response.status === 401 || response.status === 403) throw new AppError('HYPIXEL_AUTH_FAILED', 'The upstream service rejected access. Check the application and key.');
      if (response.status === 404 || response.status === 204) throw new AppError('PLAYER_NOT_FOUND', 'No matching Minecraft player was found.');
      if (!response.ok) throw new AppError('UPSTREAM_UNAVAILABLE', 'The upstream service returned an unsuccessful response.', response.status >= 500);
      const data = await limitedJson(response,requestSignal);
      if (data.success === false) throw new AppError('UPSTREAM_REJECTED', 'Hypixel rejected the request.');
      this.diagnostics.upstream(endpoint,performance.now()-started,status,undefined,authenticated?this.remaining:null);
      return data;
      } catch(error) { this.diagnostics.upstream(endpoint,performance.now()-started,status,error,authenticated?this.remaining:null); throw error; }
    };
    if (!authenticated) return send();
    // Serialize authenticated requests so headers describe the actual shared budget.
    const next = this.queue.then(send, send); this.queue = next.catch(() => {}); return next;
  }
  async resolve(player: string): Promise<Identity> {
    validatePlayer(player);
    const uuid = uuidValue(player); if (uuid) return { uuid, name: null };
    const data = await this.get(`https://api.mojang.com/users/profiles/minecraft/${encodeURIComponent(player)}`, false);
    const id = typeof data.id === 'string' ? uuidValue(data.id) : null;
    if (!id) throw new AppError('UPSTREAM_INVALID', 'Minecraft returned an invalid player identity.', true);
    return { uuid: id, name: text(data.name, 16) };
  }
  async profiles(uuid: string): Promise<JsonObject[]> {
    if (!UUID.test(uuid)) throw new AppError('INVALID_PLAYER', 'Provide a valid UUID.');
    const data = await this.get(`https://api.hypixel.net/v2/skyblock/profiles?uuid=${uuid}`, true);
    if (data.profiles === null) return [];
    if (!Array.isArray(data.profiles) || data.profiles.length > 8) throw new AppError('UPSTREAM_INVALID', 'Hypixel returned an invalid profile list.', true);
    // Discard co-op member data, banking transactions, and fields outside our contract.
    return data.profiles.map(value => {
      const p = object(value); const member = object(at(p, 'members', uuid));
      const fields = ['leveling', 'fairy_soul', 'player_data', 'currencies', 'slayer', 'dungeons', 'pets_data', 'collection', 'crafted_generators', 'inventory', 'accessory_bag_storage', 'jacobs_contest', 'garden_player_data', 'foraging'];
      const limited = Object.fromEntries(fields.filter(k => member[k] !== undefined).map(k => [k, member[k]]));
      if (member.loadout !== undefined) limited.loadout = member.loadout!==null&&typeof member.loadout==='object'&&!Array.isArray(member.loadout)?{ armor: at(member,'loadout','armor'), equipment: at(member,'loadout','equipment') }:null;
      const profileFields = ['bank_account','cookie_buff_active'];
      const ownProfile = Object.fromEntries(profileFields.filter(k=>at(member,'profile',k)!==undefined).map(k=>[k,at(member,'profile',k)]));
      if (Object.keys(ownProfile).length) limited.profile = ownProfile;
      if (member.rift !== undefined) limited.rift = { access: { consumed_prism: at(member, 'rift', 'access', 'consumed_prism') ?? false } };
      if (member.nether_island_player_data !== undefined) limited.nether_island_player_data = { abiphone: { active_contacts: at(member, 'nether_island_player_data', 'abiphone', 'active_contacts') } };
      return { profile_id: p.profile_id, cute_name: p.cute_name, selected: p.selected, game_mode: p.game_mode, members: { [uuid]: limited }, banking: p.banking===undefined ? undefined : { balance: at(p, 'banking', 'balance') } };
    });
  }
  async skillCatalog(): Promise<JsonObject> { return this.get('https://api.hypixel.net/v2/resources/skyblock/skills', false); }
  async itemCatalog(): Promise<JsonObject> {
    const raw = await this.get('https://api.hypixel.net/v2/resources/skyblock/items', false);
    if (!Array.isArray(raw.items) || raw.items.length > 20000) throw new AppError('UPSTREAM_INVALID', 'Hypixel returned an invalid item catalog.', true);
    return { lastUpdated: number(raw.lastUpdated), items: Object.fromEntries(raw.items.flatMap(value => {
      const id = text(at(value, 'id'));
      if (!id || !/^[A-Z0-9_:.-]{1,96}$/.test(id)) return [];
      return [[id, { name: text(at(value, 'name')), category: text(at(value, 'category')), tier: text(at(value, 'tier')), npcSellPrice: number(at(value, 'npc_sell_price')) }]];
    })) };
  }
  async playerMetadata(uuid: string): Promise<JsonObject> {
    if (!UUID.test(uuid)) throw new AppError('INVALID_PLAYER', 'Provide a valid UUID.');
    const raw = await this.get(`https://api.hypixel.net/v2/player?uuid=${uuid}`, true);
    if (raw.player === null) return {};
    const p = object(raw.player);
    if (!Object.keys(p).length) throw new AppError('UPSTREAM_INVALID', 'Hypixel returned invalid player metadata.', true);
    // Rank is needed for Runecrafting's cap. Never retain sessions or login history.
    return { rank: text(p.rank), packageRank: text(p.packageRank), newPackageRank: text(p.newPackageRank), monthlyPackageRank: text(p.monthlyPackageRank) };
  }
  async garden(profile: string): Promise<JsonObject | null> {
    if (!UUID.test(profile)) throw new AppError('INVALID_PROFILE', 'Provide a valid profile UUID.');
    let raw: JsonObject;
    try { raw = await this.get(`https://api.hypixel.net/v2/skyblock/garden?profile=${profile}`, true); }
    catch (error) { if (error instanceof AppError && error.code === 'PLAYER_NOT_FOUND') return null; throw error; }
    if (raw.garden === null) return null;
    const g = object(raw.garden);
    if (!Object.keys(g).length || uuidValue(String(g.uuid)) !== profile) throw new AppError('UPSTREAM_INVALID', 'Hypixel returned an invalid Garden identity.', true);
    const fields = ['garden_experience', 'resources_collected', 'crop_upgrade_levels', 'garden_upgrades', 'unlocked_plots_ids', 'selected_barn_skin', 'commission_data', 'active_commissions', 'composter_data', 'greenhouse'];
    return Object.fromEntries(fields.filter(k => g[k] !== undefined).map(k => [k, g[k]]));
  }
  async bazaar(): Promise<JsonObject> {
    const raw = await this.get('https://api.hypixel.net/v2/skyblock/bazaar', false);
    if (raw.products === null || typeof raw.products !== 'object' || Array.isArray(raw.products) || Object.keys(object(raw.products)).length > 10000) throw new AppError('UPSTREAM_INVALID', 'Hypixel returned an invalid Bazaar snapshot.', true);
    return { lastUpdated: number(raw.lastUpdated), products: raw.products };
  }
  async auctionPage(page: number, signal?: AbortSignal): Promise<JsonObject> {
    if (!Number.isInteger(page) || page < 0 || page >= 128) throw new AppError('INVALID_AUCTION_PAGE','Invalid auction page.');
    const raw = await this.get(`https://api.hypixel.net/v2/skyblock/auctions?page=${page}`,false,signal);
    if (raw.page !== page || !Array.isArray(raw.auctions) || raw.auctions.length > 1000) throw new AppError('UPSTREAM_INVALID','Invalid auction page payload.',true);
    // Retain only fields needed for listing prices and safe item identification.
    return { page: raw.page, totalPages: raw.totalPages, totalAuctions: raw.totalAuctions, lastUpdated: raw.lastUpdated, auctions: raw.auctions.map(v=>{
      const a = object(v); return { uuid: a.uuid, bin: a.bin, claimed: a.claimed, end: a.end, starting_bid: a.starting_bid, tier: a.tier, item_bytes: a.item_bytes };
    }) };
  }
}
