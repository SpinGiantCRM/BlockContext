import { Worker } from 'node:worker_threads';
import { RequestCache } from './cache.js';
import { AppError, at, number, object, type JsonObject, type Upstream } from './types.js';

type Listing = [id:string, coins:number, count:number, rarity:string, reforge:string|null, stars:number, end:number, petType:string|null];
interface Index { listings: Listing[]; updatedAt: number; retrievedAt: number; totalPages: number; scannedPages: number; totalAuctions: number; scannedAuctions: number; complete: boolean; invalidListings: number; warnings: string[] }
export interface AuctionFilters { rarity?: string; reforge?: string; stars?: number; petType?: string }
class Decoder {
  private readonly worker = new Worker(new URL('./auction-worker.js',import.meta.url),{env:{},execArgv:[],resourceLimits:{maxOldGenerationSizeMb:96,stackSizeMb:2}});
  private failed = false;
  constructor() {
    // Startup may fail while the first network page is still loading.
    this.worker.on('error',()=>{this.failed=true;});
    this.worker.on('exit',()=>{this.failed=true;});
  }
  async decode(values: unknown[], timeout: number): Promise<(JsonObject|null)[]> {
    if (this.failed) throw new AppError('AUCTION_DECODE_FAILED','Auction decoder could not start safely.',true);
    return new Promise((resolve,reject)=>{
      const failed = () => { clean(); reject(new AppError('AUCTION_DECODE_FAILED','Auction decoding exceeded safety limits.',true)); };
      const done = (result: (JsonObject|null)[]) => { clean(); resolve(result); };
      const clean = () => { clearTimeout(timer); this.worker.off('message',done); this.worker.off('error',failed); this.worker.off('exit',failed); };
      const timer = setTimeout(failed, Math.max(1,Math.min(10_000,timeout)));
      this.worker.once('message',done); this.worker.once('error',failed); this.worker.once('exit',failed);
      this.worker.postMessage(values);
    });
  }
  close() { void this.worker.terminate(); }
}

export class AuctionService {
  constructor(private readonly upstream: Upstream, private readonly cache: RequestCache, private readonly now = Date.now, private readonly budgetMs = 45_000) {}
  async quote(item: string, filters: AuctionFilters = {}): Promise<JsonObject> {
    const rarities = ['COMMON','UNCOMMON','RARE','EPIC','LEGENDARY','MYTHIC','DIVINE','SPECIAL','VERY_SPECIAL'];
    if (!/^[a-zA-Z0-9_:.-]{1,96}$/.test(item) || (filters.rarity !== undefined && !rarities.includes(filters.rarity)) || (filters.reforge !== undefined && !/^[a-z0-9_]{1,64}$/.test(filters.reforge)) || (filters.stars !== undefined && (!Number.isInteger(filters.stars) || filters.stars<0 || filters.stars>15)) || (filters.petType !== undefined && !/^[A-Z0-9_]{1,64}$/.test(filters.petType))) throw new AppError('INVALID_AUCTION_QUERY','Use an exact item ID and supported rarity, reforge, stars or pet type filters.');
    const id = item.toUpperCase();
    if (filters.petType !== undefined && id !== 'PET') throw new AppError('INVALID_AUCTION_QUERY','petType is supported only for item PET.');
    const snapshot = await this.cache.get(`v5:${this.upstream.source}:auction-index`,60_000,180_000,()=>this.scan(),v=>Math.min(v.retrievedAt,v.updatedAt));
    const data = snapshot.value; this.validateTime(data.updatedAt);
    const matches = data.listings.filter(([itemId,,,,,,,petType]) => itemId===id && (filters.petType===undefined || petType===filters.petType)).filter(l=>l[6]>this.now() && (filters.rarity===undefined || l[3]===filters.rarity) && (filters.reforge===undefined || l[4]===filters.reforge) && (filters.stars===undefined || l[5]===filters.stars));
    const sorted = matches.sort((a,b)=>a[1]-b[1]);
    const warnings = [...data.warnings,'Active BIN asking prices are not completed sales or a valuation of your owned item. Enchantments, upgrades beyond stars, attributes, skins, pet level and other variants can differ. Listings can be bought or cancelled after the snapshot. Gross SkyBlock coins exclude fees.'];
    if (!data.complete) warnings.push('The scan is incomplete. Only observed prices are available; the global lowest BIN price is unknown.');
    if (snapshot.warning) warnings.push(snapshot.warning);
    if (this.upstream.source==='synthetic_demo') warnings.unshift('SYNTHETIC DEMO: this auction snapshot is invented.');
    const lowestObserved = sorted[0]?.[1] ?? null;
    return {
      metadata: { source: this.upstream.source, retrievedAt:new Date(data.retrievedAt).toISOString(), upstreamUpdatedAt:new Date(data.updatedAt).toISOString(), ageSeconds:Math.max(0,Math.floor((this.now()-data.updatedAt)/1000)), cache:snapshot.cache, stale:snapshot.cache==='stale', freshnessPolicy:{refreshAfterSeconds:60,maximumSnapshotAgeSeconds:180,refreshOnlyOnRequest:true}, warnings },
      item:{id}, filters, currency:'SkyBlock coins', feesIncluded:false, priceKind:'active_bin_asking_price',
      coverage:{status:data.complete?'complete':'partial',scannedPages:data.scannedPages,totalPages:data.totalPages,scannedAuctions:data.scannedAuctions,totalAuctions:data.totalAuctions,invalidListings:data.invalidListings},
      matchedListingCount:matches.length, lowestBinCoins:data.complete?lowestObserved:null, lowestObservedBinCoins:lowestObserved,
      lowestUnitCoins:data.complete && matches.length ? matches.reduce((minimum,l)=>Math.min(minimum,l[1]/l[2]),Infinity):null,
      listings:sorted.slice(0,5).map(l=>({coins:l[1],itemCount:l[2],unitCoins:l[1]/l[2],rarity:l[3],reforge:l[4],stars:l[5],endsAt:new Date(l[6]).toISOString(),petType:l[7]}))
    };
  }
  private validateTime(value: unknown) {
    const t = number(value);
    if (t===null || t>this.now()+10_000 || this.now()-t>=180_000) throw new AppError('AUCTION_SNAPSHOT_EXPIRED','The upstream auction snapshot is not recent enough.',true);
  }
  private async scan(): Promise<Index> {
    const controller = new AbortController(), started = Date.now(), deadline = started+this.budgetMs;
    const timer = setTimeout(()=>controller.abort(),this.budgetMs), decoder = new Decoder();
    const index: Index = {listings:[],updatedAt:0,retrievedAt:this.now(),totalPages:0,scannedPages:0,totalAuctions:0,scannedAuctions:0,complete:true,invalidListings:0,warnings:[]};
    const seen = new Set<string>();
    const consume = async (raw: JsonObject, expected: number) => {
      this.validateTime(raw.lastUpdated);
      if (raw.page!==expected || !Number.isInteger(raw.totalPages) || Number(raw.totalPages)<1 || Number(raw.totalPages)>128 || !Number.isInteger(raw.totalAuctions) || Number(raw.totalAuctions)<0 || Number(raw.totalAuctions)>128000 || !Array.isArray(raw.auctions) || raw.auctions.length>1000) throw new AppError('UPSTREAM_INVALID','Auction page metadata is invalid.',true);
      if (expected===0) { index.updatedAt=Number(raw.lastUpdated); index.totalPages=Number(raw.totalPages); index.totalAuctions=Number(raw.totalAuctions); }
      else if (raw.lastUpdated!==index.updatedAt || raw.totalPages!==index.totalPages || raw.totalAuctions!==index.totalAuctions) { index.complete=false; index.warnings.push('Auction pages changed during the scan; pages from another snapshot were omitted.'); return false; }
      index.scannedPages++; index.scannedAuctions+=raw.auctions.length;
      const bins = raw.auctions.map(object).filter(a=>{
        if (typeof a.uuid!=='string' || !/^[0-9a-f]{32}$/i.test(a.uuid) || seen.has(a.uuid)) {index.complete=false; index.invalidListings++; return false;}
        seen.add(a.uuid);
        if (typeof a.bin!=='boolean' || typeof a.claimed!=='boolean' || number(a.end)===null) {index.complete=false;index.invalidListings++;return false;}
        return a.bin===true && a.claimed===false && Number(a.end)>this.now();
      });
      const parsed = await decoder.decode(bins.map(a=>typeof a.item_bytes==='string'?a.item_bytes:at(a.item_bytes,'data')),deadline-Date.now());
      bins.forEach((a,i)=>{
        const item = parsed[i];
        if (!item || number(a.starting_bid)===null || Number(a.starting_bid)<=0 || typeof a.tier!=='string' || !/^[A-Z_ ]{1,16}$/.test(a.tier)) {index.complete=false;index.invalidListings++;return;}
        index.listings.push([String(item.id),Number(a.starting_bid),Number(item.count),a.tier.replaceAll(' ','_'),item.reforge as string|null,Number(item.stars),Number(a.end),item.petType as string|null]);
      });
      return true;
    };
    try {
      await consume(await this.upstream.auctionPage(0,controller.signal),0);
      // Two simultaneous network reads, one bounded parser, and a finite scan.
      for (let page=1;page<index.totalPages;page+=2) {
        if (Date.now()>=deadline) throw new AppError('AUCTION_SCAN_TIMEOUT','Auction scan reached its time budget.',true);
        const pages = [page,...(page+1<index.totalPages?[page+1]:[])];
        const results = await Promise.allSettled(pages.map(p=>this.upstream.auctionPage(p,controller.signal)));
        let stop = false;
        for (let i=0;i<results.length;i++) {
          const result=results[i]!;
          if (result.status==='rejected') { index.complete=false; index.warnings.push('One or more auction pages could not be retrieved.'); stop=true; }
          else if (!await consume(result.value,pages[i]!)) stop=true;
        }
        if (stop) break;
      }
    } catch (error) {
      if (!index.scannedPages) throw error;
      index.complete=false;index.warnings.push('The scan stopped before complete coverage could be established.');
    } finally { clearTimeout(timer); controller.abort(); decoder.close(); }
    if (index.scannedPages!==index.totalPages || index.scannedAuctions!==index.totalAuctions) index.complete=false;
    index.retrievedAt=this.now(); this.validateTime(index.updatedAt);
    return index;
  }
}
