#!/usr/bin/env python3
"""Summarize BlockContext operational JSONL without displaying individual requests."""
import argparse, collections, datetime, json, math, pathlib, time
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--hours',type=float,default=24)
parser.add_argument('--directory',type=pathlib.Path,default=pathlib.Path('/var/log/blockcontext'))
args=parser.parse_args()
if not 0 < args.hours <= 336: parser.error('--hours must be greater than zero and no more than 336')
cutoff=time.time()-args.hours*3600
rows=[];invalid=0
for path in sorted(args.directory.glob('usage.jsonl*')):
 if path.name!='usage.jsonl' and not path.name.removeprefix('usage.jsonl.').isdigit(): continue
 if path.is_symlink() or not path.is_file(): continue
 with path.open() as handle:
  for line in handle:
   try:
    row=json.loads(line); timestamp=datetime.datetime.fromisoformat(row['time'].replace('Z','+00:00')).timestamp()
    if timestamp>=cutoff: rows.append(row)
   except (ValueError,KeyError,TypeError): invalid+=1

def timing(values):
 values=sorted(v for v in values if isinstance(v,(int,float)))
 def q(p): return values[max(0,math.ceil(p*len(values))-1)] if values else None
 return {'count':len(values),'p50Ms':q(.5),'p95Ms':q(.95),'maxMs':max(values,default=None)}
def grouped(event,field):
 groups=collections.defaultdict(list)
 for row in rows:
  if row.get('event')==event: groups[row.get(field) or 'unknown'].append(row)
 return groups
out={'windowHours':args.hours,'events':len(rows),'invalidLines':invalid,'tools':{},'upstream':{}}
for name,group in grouped('tool','tool').items():
 out['tools'][name]={**timing([r.get('durationMs') for r in group]),'outcomes':dict(collections.Counter(r.get('outcome') for r in group)),'cache':dict(collections.Counter(r.get('cache') for r in group if r.get('cache'))),'errors':dict(collections.Counter(r.get('errorCode') for r in group if r.get('errorCode'))),'auctionCoverage':dict(collections.Counter(r.get('auctionCoverage') for r in group if r.get('auctionCoverage'))),'auctionReasons':dict(collections.Counter(reason for r in group for reason in r.get('auctionReasons',[])))}
for name,group in grouped('upstream','endpoint').items():
 out['upstream'][name]={**timing([r.get('durationMs') for r in group]),'httpStatuses':dict(collections.Counter(str(r.get('httpStatus')) for r in group)),'errors':dict(collections.Counter(r.get('errorCode') for r in group if r.get('errorCode'))),'lastRemainingRequests':next((r['remainingRequests'] for r in reversed(sorted(group,key=lambda r:r['time'])) if r.get('remainingRequests') is not None),None)}
resource=sorted((r for r in rows if r.get('event')=='resource'),key=lambda r:r['time'])
out['resources']={'samples':len(resource),'peakRssMiB':round(max((r.get('rssBytes',0) for r in resource),default=0)/1048576,1),'droppedMeasurements':max((r.get('droppedMeasurements',0) for r in resource),default=0)}
cpu=[]
for prev,current in zip(resource,resource[1:]):
 elapsed=current['uptimeSeconds']-prev['uptimeSeconds']
 used=(current['cpuUserMicroseconds']+current['cpuSystemMicroseconds'])-(prev['cpuUserMicroseconds']+prev['cpuSystemMicroseconds'])
 if elapsed>0 and used>=0: cpu.append(round(used/elapsed/10000,2))
out['resources']['peakCpuPercentOfOneCore']=max(cpu,default=None)
out['httpStatuses']=dict(collections.Counter(str(r.get('httpStatus')) for r in rows if r.get('event')=='http'))
print(json.dumps(out,indent=2,sort_keys=True))
