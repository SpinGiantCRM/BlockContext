import { appendFileSync, chmodSync, lstatSync, mkdirSync, readFileSync, readdirSync, renameSync, unlinkSync } from 'node:fs';
import { dirname, basename, join } from 'node:path';
import { AppError, at, number, SECTIONS, type JsonObject } from './types.js';

const tools = ['skyblock_profile_context','skyblock_profile_section','skyblock_market_quote','skyblock_auction_quote'];
const endpoints = ['identity','profiles','skills','items','rank','garden','bazaar','auctions'];
const codes = new Set(['AUCTION_DECODE_FAILED','AUCTION_SCAN_TIMEOUT','AUCTION_SNAPSHOT_EXPIRED','INVALID_AUCTION_QUERY','UPSTREAM_INVALID','INTERNAL_ERROR','RESPONSE_TOO_LARGE','SNAPSHOT_EXPIRED','DEMO_PLAYER_ONLY','INVENTORY_INVALID','INVALID_MARKET_QUERY','MARKET_ITEM_NOT_FOUND','MARKET_SNAPSHOT_EXPIRED','INVALID_PAGE','INVALID_PROFILE','INVALID_SECTION','NO_SKYBLOCK_PROFILES','PROFILE_REQUIRED','PROFILE_AMBIGUOUS','PROFILE_NOT_FOUND','HYPIXEL_AUTH_FAILED','HYPIXEL_KEY_EXPIRED','HYPIXEL_KEY_REQUIRED','INVALID_AUCTION_PAGE','INVALID_CONFIG','INVALID_PLAYER','PLAYER_NOT_FOUND','RATE_LIMITED','UPSTREAM_REJECTED','UPSTREAM_TOO_LARGE','UPSTREAM_UNAVAILABLE']);
const count = (v: unknown) => { const n=number(v); return n===null ? null : Math.max(0,Math.min(1e9,Math.round(n))); };
const choice = (v: unknown, allowed: readonly string[]) => typeof v==='string' && allowed.includes(v) ? v : null;

// Only operational measurements are accepted. Never pass arguments, URLs, keys,
// player identities, returned profile data or arbitrary exception text to a sink.
export class Diagnostics {
  private lastCleanup=0;
  private lastFailure=-Infinity;
  private dropped=0;
  private fileStarted:number|undefined;
  constructor(private readonly path?: string, private readonly options: { maxBytes?:number; archives?:number; retentionMs?:number; now?:()=>number }={}) {}
  private now() { return this.options.now?.() ?? Date.now(); }
  private emit(event: string, fields: JsonObject) {
    if (!this.path) return;
    try {
      const dir=dirname(this.path), base=basename(this.path), now=this.now();
      mkdirSync(dir,{recursive:true,mode:0o700});
      if (!this.lastCleanup || now-this.lastCleanup>=3600_000) {
        const retention=this.options.retentionMs ?? 14*86400_000;
        for (const name of readdirSync(dir)) if (name===base || new RegExp('^'+base.replace(/[.*+?^${}()|[\]\\]/g,'\\$&')+'\\.[1-9][0-9]*$').test(name)) {
          const file=join(dir,name), stat=lstatSync(file);
          if (!stat.isFile() || stat.nlink!==1) throw new Error('Unsafe diagnostic file');
          if (now-stat.mtimeMs>=retention) { unlinkSync(file); if (name===base) this.fileStarted=undefined; }
        }
        this.lastCleanup=now;
      }
      const line=JSON.stringify({schema:1,time:new Date(now).toISOString(),event,...fields})+'\n';
      let size=0;
      try { const stat=lstatSync(this.path); if (!stat.isFile() || stat.nlink!==1) throw new Error('Unsafe diagnostic file'); size=stat.size; }
      catch (error) { if ((error as NodeJS.ErrnoException).code!=='ENOENT') throw error; }
      if (size && this.fileStarted===undefined) {
        const first=JSON.parse(readFileSync(this.path,'utf8').split('\n',1)[0]!);
        const started=Date.parse(first.time); if (!Number.isFinite(started)) throw new Error('Invalid diagnostic file');
        this.fileStarted=started;
      }
      if (size && (size+Buffer.byteLength(line)>(this.options.maxBytes ?? 1024*1024) || now-(this.fileStarted ?? now)>=86400_000)) {
        const archives=this.options.archives ?? 13;
        for (let i=archives;i>=1;i--) {
          const from=i===1 ? this.path : this.path+'.'+(i-1), to=this.path+'.'+i;
          if (i===archives) { try { unlinkSync(to); } catch (error) { if ((error as NodeJS.ErrnoException).code!=='ENOENT') throw error; } }
          try { renameSync(from,to); } catch (error) { if ((error as NodeJS.ErrnoException).code!=='ENOENT') throw error; }
        }
        this.fileStarted=undefined;
      }
      appendFileSync(this.path,line,{mode:0o600}); chmodSync(this.path,0o600);
      this.fileStarted ??= now;
    } catch {
      this.dropped++;
      if (this.now()-this.lastFailure>=60_000) { this.lastFailure=this.now(); console.error('BlockContext diagnostics unavailable; measurements dropped.'); }
    }
  }
  tool(tool:string, durationMs:number, data?:JsonObject, error?:unknown, section?:string, responseBytes?:number) {
    const metadata=at(data,'metadata'), coverage=at(data,'coverage');
    const safeError=error instanceof AppError && codes.has(error.code) ? error.code : 'INTERNAL_ERROR';
    const warnings=at(metadata,'warnings');
    const auctionReasons=Object.entries({snapshot_changed:'Auction pages changed during the scan; pages from another snapshot were omitted.',page_fetch_failed:'One or more auction pages could not be retrieved.',scan_stopped:'The scan stopped before complete coverage could be established.'}).filter(([,message])=>Array.isArray(warnings)&&warnings.includes(message)).map(([reason])=>reason);
    if (Number(at(coverage,'unknownVariantListingCount'))>0) auctionReasons.push('unknown_variants');
    if (Number(at(coverage,'invalidListings'))>0) auctionReasons.push('invalid_listings');
    this.emit('tool',{tool:choice(tool,tools),section:choice(section,SECTIONS),durationMs:count(durationMs),outcome:error?'error':'ok',errorCode:error?safeError:null,retryAfterSeconds:error instanceof AppError?count(error.retryAfterSeconds):null,cache:choice(at(metadata,'cache'),['fresh','miss','stale']),snapshotAgeSeconds:count(at(metadata,'ageSeconds')),warningCount:Array.isArray(warnings)?warnings.length:0,responseBytes:count(responseBytes),sectionStatus:choice(at(data,'result','status'),['available','partial','unavailable']),auctionCoverage:choice(at(coverage,'status'),['complete','partial']),auctionReasons,scannedPages:count(at(coverage,'scannedPages')),totalPages:count(at(coverage,'totalPages')),scannedAuctions:count(at(coverage,'scannedAuctions')),invalidListings:count(at(coverage,'invalidListings')),unknownVariantListings:count(at(coverage,'unknownVariantListingCount'))});
  }
  upstream(endpoint:string, durationMs:number, status:number|null, error?:unknown, remaining?:number|null) {
    this.emit('upstream',{endpoint:choice(endpoint,endpoints),durationMs:count(durationMs),httpStatus:count(status),outcome:error?'error':'ok',errorCode:error ? error instanceof AppError && codes.has(error.code)?error.code:'INTERNAL_ERROR':null,remainingRequests:count(remaining)});
  }
  http(status:number, durationMs:number, active:number) { this.emit('http',{httpStatus:count(status),durationMs:count(durationMs),activeRequests:count(active)}); }
  lifecycle(state:'started'|'stopping',mode:string,transport:string) { this.emit('lifecycle',{state,mode:choice(mode,['demo','live']),transport:choice(transport,['stdio','http'])}); }
  resource() {
    const memory=process.memoryUsage(), cpu=process.cpuUsage();
    this.emit('resource',{uptimeSeconds:count(process.uptime()),rssBytes:memory.rss,heapUsedBytes:memory.heapUsed,externalBytes:memory.external,cpuUserMicroseconds:cpu.user,cpuSystemMicroseconds:cpu.system,droppedMeasurements:this.dropped});
  }
}
