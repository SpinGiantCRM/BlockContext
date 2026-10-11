import { AppError, at, number, object, text } from './types.js';
function book(value, ascending) {
    if (!Array.isArray(value) || value.length > 30)
        return null;
    const rows = value.map(object);
    if (rows.some(r => number(r.amount) === null || Number(r.amount) <= 0 || !Number.isInteger(r.amount) || number(r.pricePerUnit) === null || Number(r.pricePerUnit) <= 0))
        return null;
    return rows.map(r => ({ amount: r.amount, pricePerUnit: r.pricePerUnit })).sort((a, b) => ascending ? Number(a.pricePerUnit) - Number(b.pricePerUnit) : Number(b.pricePerUnit) - Number(a.pricePerUnit));
}
function fill(rows, quantity) {
    if (rows === null)
        return { status: 'unavailable', bestUnitPrice: null, totalCoins: null, filledQuantity: null, remainingQuantity: quantity, averageUnitPrice: null, visibleDepth: null };
    let remaining = quantity, coins = 0;
    for (const row of rows) {
        const take = Math.min(remaining, Number(row.amount));
        coins += take * Number(row.pricePerUnit);
        remaining -= take;
        if (!remaining)
            break;
    }
    const filled = quantity - remaining;
    return { status: remaining ? 'partial' : 'available', bestUnitPrice: number(rows[0]?.pricePerUnit), totalCoins: remaining ? null : coins, visibleFillCoins: filled ? coins : null, filledQuantity: filled, remainingQuantity: remaining, averageUnitPrice: filled ? coins / filled : null, visibleDepth: rows.reduce((n, r) => n + Number(r.amount), 0) };
}
export class MarketService {
    upstream;
    cache;
    now;
    constructor(upstream, cache, now = Date.now) {
        this.upstream = upstream;
        this.cache = cache;
        this.now = now;
    }
    async quote(item, quantity = 1) {
        if (!/^[a-zA-Z0-9_:.-]{1,96}$/.test(item) || !Number.isInteger(quantity) || quantity < 1 || quantity > 1_000_000)
            throw new AppError('INVALID_MARKET_QUERY', 'Provide an exact item ID and a whole-number quantity from 1 to 1000000.');
        const id = item.toUpperCase(), prefix = `v5:${this.upstream.source}`;
        const snapshot = await this.cache.get(`${prefix}:bazaar`, 45_000, 180_000, async () => {
            const data = await this.upstream.bazaar(), retrievedAt = this.now();
            this.validateTime(data.lastUpdated);
            return { data, retrievedAt };
        }, value => value.retrievedAt);
        this.validateTime(snapshot.value.data.lastUpdated);
        let catalog = {};
        try {
            catalog = (await this.cache.get(`${prefix}:item-catalog`, 24 * 3600_000, 48 * 3600_000, () => this.upstream.itemCatalog())).value;
        }
        catch { /* NPC value and display name stay unknown. */ }
        const product = at(snapshot.value.data, 'products', id), definition = object(at(catalog, 'items', id));
        if (product === undefined && !Object.keys(definition).length)
            throw new AppError('MARKET_ITEM_NOT_FOUND', 'This item ID is absent from the Bazaar and available item catalog.');
        const p = object(product);
        // Hypixel calls asks buy_summary (instant buy) and bids sell_summary (instant sell).
        const instantBuy = fill(book(p.buy_summary, true), quantity), instantSell = fill(book(p.sell_summary, false), quantity);
        const warnings = ['Quotes use the visible top 30 order levels; they are not guaranteed execution prices. All coin values are gross and exclude taxes and fees. No trading or gameplay action is performed.'];
        if (snapshot.warning)
            warnings.push(snapshot.warning);
        if (this.upstream.source === 'synthetic_demo')
            warnings.unshift('SYNTHETIC DEMO: these are invented market prices.');
        if (instantBuy.status !== 'available' || instantSell.status !== 'available')
            warnings.push('A missing or incomplete order book cannot supply a complete quote for this quantity.');
        const npc = number(definition.npcSellPrice);
        return {
            metadata: { source: this.upstream.source, retrievedAt: new Date(snapshot.value.retrievedAt).toISOString(), upstreamUpdatedAt: new Date(Number(snapshot.value.data.lastUpdated)).toISOString(), ageSeconds: Math.max(0, Math.floor((this.now() - snapshot.value.retrievedAt) / 1000)), upstreamAgeSeconds: Math.max(0, Math.floor((this.now() - Number(snapshot.value.data.lastUpdated)) / 1000)), cache: snapshot.cache, stale: snapshot.cache === 'stale', freshnessPolicy: { refreshAfterSeconds: 45, maximumSnapshotAgeSeconds: 180, refreshOnlyOnRequest: true }, warnings },
            item: { id, name: text(definition.name) }, quantity, currency: 'SkyBlock coins', feesIncluded: false,
            bazaar: product === undefined ? { status: 'unavailable', reason: 'ITEM_NOT_TRADED_ON_BAZAAR', instantBuy: null, instantSell: null } : { status: instantBuy.status === 'available' && instantSell.status === 'available' ? 'available' : 'partial', instantBuy, instantSell, quickStatusWeightedPrices: { buy: number(at(p, 'quick_status', 'buyPrice')), sell: number(at(p, 'quick_status', 'sellPrice')) }, volumes: { buy: number(at(p, 'quick_status', 'buyVolume')), sell: number(at(p, 'quick_status', 'sellVolume')), boughtMovingWeek: number(at(p, 'quick_status', 'buyMovingWeek')), soldMovingWeek: number(at(p, 'quick_status', 'sellMovingWeek')) } },
            npcSell: { unitCoins: npc, totalCoins: npc === null ? null : npc * quantity, catalogLastUpdated: number(catalog.lastUpdated) },
            auctionPrice: null, auctionCoverage: 'Not included; item variants cannot be valued from Bazaar prices.'
        };
    }
    validateTime(value) {
        const timestamp = number(value);
        if (timestamp === null || timestamp > this.now() + 10_000 || this.now() - timestamp >= 180_000)
            throw new AppError('MARKET_SNAPSHOT_EXPIRED', 'The upstream market snapshot is missing a valid recent timestamp. Try again later.', true);
    }
}
