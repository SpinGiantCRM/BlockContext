#!/usr/bin/env python3
"""Summarize private BlockContext logs by runtime/version without player data."""
import argparse, collections, datetime, json, math, pathlib, re, time
TOOLS={'skyblock_profile_context','skyblock_profile_section','skyblock_market_quote','skyblock_auction_quote','skyblock_client_context','skyblock_live_context','skyblock_reference','skyblock_poll_schedule'}
ENDPOINTS={'identity','profiles','skills','items','rank','garden','bazaar','auctions','collections','museum','bingo','own_auctions','election','news','fire_sales','ended_auctions'}
STAGES={'cache_read','cache_write','profile_normalize','client_read','rpc_payload_serialize','cache_lock_wait','sqlite_read','sqlite_write','cache_json_parse','cache_json_serialize','cpu_queue_wait','worker_queue_wait','field_map','inventory_decode','gzip_expand','nbt_validate','nbt_parse','client_file_read','client_validate','upstream_budget_wait','upstream_headers','upstream_body','upstream_json_parse','auction_decode','bridge_project','bridge_ssh','resource_tick_lag'}
COUNTERS={'cache_hits','cache_misses','cache_shared_loads','cache_cooldowns','cache_stale_fallbacks','cache_expired_removed','cache_capacity_removed','cache_read_bytes','cache_write_bytes','upstream_requests','upstream_body_bytes','inventory_input_bytes','inventory_expanded_bytes','inventory_decode_failures','client_snapshot_bytes','bridge_unchanged','bridge_uploads','bridge_upload_bytes','bridge_upload_failures','bridge_snapshot_failures','bridge_waiting'}

def number(v):
    return isinstance(v,(int,float)) and not isinstance(v,bool) and math.isfinite(v) and v>=0

def timing(values):
    values=sorted(v for v in values if number(v))
    def q(p):return values[max(0,math.ceil(p*len(values))-1)] if values else None
    return {'count':len(values),'p50Ms':q(.5),'p95Ms':q(.95),'p99Ms':q(.99),'maxMs':max(values,default=None)}

def safe(value,allowed):return value if isinstance(value,str) and value in allowed else 'unknown'
def code(value):return value if isinstance(value,str) and re.fullmatch(r'[A-Z][A-Z0-9_]{0,63}',value) else 'unknown'
def runtime(row):
    if row.get('runtime') in ('rust','node'):return row['runtime']
    if row.get('event')=='resource' and number(row.get('heapUsedBytes')):return 'node'
    return None

def classify(rows):
    active='unknown';version='unknown';epoch=0
    for row in rows:
        if row.get('event')=='lifecycle' and row.get('state')=='started':epoch+=1;active='unknown';version='unknown'
        active=runtime(row) or active
        v=row.get('runtimeVersion')
        if isinstance(v,str) and re.fullmatch(r'\d+\.\d+\.\d+(?:-[a-z0-9.]+)?',v):version=v
        row['_runtime']=active;row['_version']=version
        start=row.get('processStartedAt'); row['_epoch']=(active,version,safe(row.get('component'),{'backend','bridge'}),start if isinstance(start,str) and len(start)<=64 else epoch)
    return rows

def summarize(rows):
    out={'events':len(rows),'tools':{},'upstream':{},'stages':{},'stageTimingsAreOverlapping':True,'counters':{}}
    def grouped(event,key,allowed):
        groups=collections.defaultdict(list)
        for r in rows:
            if r.get('event')==event:groups[safe(r.get(key),allowed)].append(r)
        return groups
    for name,group in grouped('tool','tool',TOOLS).items():
        stats={**timing([r.get('durationMs') for r in group]),'outcomes':dict(collections.Counter(safe(r.get('outcome'),{'ok','error'}) for r in group)),'errors':dict(collections.Counter(code(r['errorCode']) for r in group if r.get('errorCode')))}
        for field,allowed,dest in [('cache',{'fresh','miss','stale'},'byCache'),('workload',{'synthetic_demo','api_snapshot','client_snapshot'},'byWorkload')]:
            groups=collections.defaultdict(list)
            for r in group:groups[safe(r.get(field),allowed)].append(r.get('durationMs'))
            stats[dest]={k:timing(v) for k,v in groups.items()}
        stats['cache']=dict(collections.Counter(safe(r.get('cache'),{'fresh','miss','stale'}) for r in group))
        stats['responseBytes']=timing([r.get('responseBytes') for r in group])
        # Byte sizes have byte units, not millisecond labels.
        stats['responseBytes']={k.replace('Ms','Bytes'):v for k,v in stats['responseBytes'].items()}
        stats['auctionCoverage']=dict(collections.Counter(safe(r.get('auctionCoverage'),{'complete','partial'}) for r in group if r.get('auctionCoverage')))
        stats['auctionReasons']=dict(collections.Counter(safe(reason,{'snapshot_changed','page_fetch_failed','scan_stopped','unknown_variants','invalid_listings'}) for r in group for reason in (r.get('auctionReasons') or []) if isinstance(reason,str)))
        stats['auctionFetches']={field:sum(r.get(field) for r in group if r.get('cache')=='miss' and number(r.get(field))) for field in ['retryAttempts','recoveredPageFetches','failedPageFetches']}
        out['tools'][name]=stats
    for name,group in grouped('upstream','endpoint',ENDPOINTS).items():
        out['upstream'][name]={**timing([r.get('durationMs') for r in group]),'httpStatuses':dict(collections.Counter(str(r.get('httpStatus')) if number(r.get('httpStatus')) else 'unknown' for r in group)),'errors':dict(collections.Counter(code(r['errorCode']) for r in group if r.get('errorCode'))),'lastRemainingRequests':next((r['remainingRequests'] for r in reversed(group) if number(r.get('remainingRequests'))),None)}
    for name,group in grouped('stage','stage',STAGES).items():
        samples=[];calls=0;omitted=0
        for r in group:
            values=r.get('durationsMs')
            if isinstance(values,list) and len(values)<=1024:samples.extend(values)
            else:samples.append(r.get('durationMs'))
            calls+=r['calls'] if number(r.get('calls')) else 1
            omitted+=r['omittedSamples'] if number(r.get('omittedSamples')) else 0
        measured=timing(samples); maxima=[r['maximumDurationMs'] for r in group if number(r.get('maximumDurationMs'))]; measured['maxMs']=max(maxima+[measured['maxMs']],default=None) if measured['maxMs'] is not None else max(maxima,default=None)
        out['stages'][name]={**measured,'totalCalls':calls,'omittedSamples':omitted,'coverage':'sample_limited' if omitted else 'complete'}
    for row in rows:
        if row.get('event')=='counters' and isinstance(row.get('values'),dict):
            for key,value in row['values'].items():
                if key in COUNTERS and number(value):out['counters'][key]=out['counters'].get(key,0)+value
    resources=[r for r in rows if r.get('event')=='resource'];rss=[r['rssBytes']/1048576 for r in resources if number(r.get('rssBytes'))]
    cpu=[];previous={}
    for current in resources:
        epoch=current['_epoch'];prev=previous.get(epoch);previous[epoch]=current
        fields=['uptimeSeconds','cpuUserMicroseconds','cpuSystemMicroseconds']
        if not prev or not all(number(r.get(k)) for r in (prev,current) for k in fields):continue
        elapsed=current['uptimeSeconds']-prev['uptimeSeconds'];used=sum(current[k]-prev[k] for k in fields[1:])
        if elapsed>0 and used>=0:cpu.append(round(used/elapsed/10000,2))
    out['resources']={'samples':len(resources),'rssSamples':len(rss),'peakRssMiB':round(max(rss),1) if rss else None,'cpuIntervals':len(cpu),'peakCpuPercentOfOneCore':max(cpu,default=None),'droppedMeasurements':max((r['droppedMeasurements'] for r in resources if number(r.get('droppedMeasurements'))),default=0)}
    for field in ['peakRssBytes','threads','openFileDescriptors','readBytes','writeBytes','voluntaryContextSwitches','involuntaryContextSwitches']:
        values=[r[field] for r in resources if number(r.get(field))]
        out['resources'][field]={'samples':len(values),'maximum':max(values,default=None)}

    out['httpStatuses']=dict(collections.Counter(str(r.get('httpStatus')) if number(r.get('httpStatus')) else 'unknown' for r in rows if r.get('event')=='http'))
    return out

def load(directory,cutoff):
    rows=[];invalid=0
    for path in sorted(directory.glob('usage.jsonl*')):
        if path.name!='usage.jsonl' and not path.name.removeprefix('usage.jsonl.').isdigit():continue
        if path.is_symlink() or not path.is_file():continue
        with path.open() as handle:
            for line in handle:
                try:
                    row=json.loads(line);row['_time']=datetime.datetime.fromisoformat(row['time'].replace('Z','+00:00')).timestamp()
                    rows.append(row)
                except (ValueError,KeyError,TypeError):invalid+=1
    rows=classify(sorted(rows,key=lambda r:r['_time']))
    return [r for r in rows if r['_time']>=cutoff],invalid

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--hours',type=float,default=24);p.add_argument('--directory',type=pathlib.Path,default=pathlib.Path('/var/log/blockcontext'));p.add_argument('--runtime',choices=['rust','node','unknown']);p.add_argument('--component',choices=['backend','bridge','unknown']);a=p.parse_args()
    if not math.isfinite(a.hours) or not 0<a.hours<=336:p.error('--hours must be greater than zero and no more than 336')
    rows,invalid=load(a.directory,time.time()-a.hours*3600)
    if a.runtime:rows=[r for r in rows if r['_runtime']==a.runtime]
    if a.component:rows=[r for r in rows if safe(r.get('component'),{'backend','bridge'})==a.component]
    out={'windowHours':a.hours,'invalidLines':invalid,**summarize(rows),'byRuntime':{},'byComponent':{}}
    groups=collections.defaultdict(list)
    for r in rows:groups[r['_runtime']+'@'+r['_version']].append(r)
    for k,group in groups.items():out['byRuntime'][k]=summarize(group)
    components=collections.defaultdict(list)
    for r in rows:components[safe(r.get('component'),{'backend','bridge'})].append(r)
    for k,group in components.items():out['byComponent'][k]=summarize(group)
    print(json.dumps(out,indent=2,sort_keys=True))
if __name__=='__main__':main()
