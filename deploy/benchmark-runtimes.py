#!/usr/bin/env python3
"""Compare explicit demo MCP runtimes on one host; never call a live instance."""
import argparse, http.client, json, math, os, pathlib, statistics, subprocess, tempfile, time

def request(port, method, params=None):
    peer=http.client.HTTPConnection('127.0.0.1',port,timeout=30)
    try:
        body=json.dumps({'jsonrpc':'2.0','id':1,'method':method,'params':params or {}})
        peer.request('POST','/mcp',body,{'Content-Type':'application/json','Accept':'application/json, text/event-stream'})
        response=peer.getresponse(); raw=response.read();assert response.status==200, 'Demo transport failed'
        rpc=json.loads(raw);assert not rpc.get('error') and not rpc.get('result',{}).get('isError'), 'Demo tool failed'
        return rpc['result']
    finally:peer.close()

def resource(pid):
    try:
        rows=pathlib.Path(f'/proc/{pid}/stat').read_text().rsplit(')',1)[1].split()
        cpu=(int(rows[11])+int(rows[12]))/os.sysconf('SC_CLK_TCK')
        status=pathlib.Path(f'/proc/{pid}/status').read_text().splitlines()
        peak=next(int(x.split()[1])*1024 for x in status if x.startswith('VmHWM:'))
        return cpu,peak
    except (OSError,StopIteration,ValueError):return None,None

def run(kind,args,logging):
    port=3036 if kind=='node' else 3037
    with tempfile.TemporaryDirectory(prefix='blockcontext-offline-bench-') as folder:
        folder=pathlib.Path(folder); env={k:v for k,v in os.environ.items() if k in ['PATH','SYSTEMROOT','WINDIR','TEMP','TMP']}
        env.update(BLOCKCONTEXT_MODE='demo',BLOCKCONTEXT_TRANSPORT='http',PORT=str(port),BLOCKCONTEXT_DB=str(folder/'cache.sqlite'))
        if logging:env['BLOCKCONTEXT_LOG_PATH']=str(folder/'usage.jsonl')
        command=[args.node,args.node_main] if kind=='node' else [args.rust,'--mode','demo','--transport','http','--port',str(port),'--db',str(folder/'cache.sqlite')]
        start=time.perf_counter(); process=subprocess.Popen(command,cwd=folder,env=env,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
        try:
            ready=False
            for _ in range(1000):
                if process.poll() is not None:raise RuntimeError('Demo runtime exited before readiness')
                try:
                    peer=http.client.HTTPConnection('127.0.0.1',port,timeout=.1);peer.request('GET','/health');r=peer.getresponse();health=json.loads(r.read());peer.close()
                    assert health.get('mode')=='demo','Refusing benchmark against live mode'
                    ready=True;break
                except (ConnectionError,OSError):time.sleep(.01)
            assert ready,'Demo startup timed out'
            startup=(time.perf_counter()-start)*1000
            def call(tool,extra):
                t=time.perf_counter();r=request(port,'tools/call',{'name':tool,'arguments':{'player':'DemoPlayer','profile':'Strawberry',**extra}})
                assert r['structuredContent']['metadata']['source']=='synthetic_demo','Only invented fixtures are allowed'
                return (time.perf_counter()-t)*1000
            cold=call('skyblock_profile_context',{'mode':'snapshot'})
            for _ in range(4):call('skyblock_profile_context',{'mode':'cached'})
            phases={}; cpu0,_=resource(process.pid)
            for name,tool,extra,n in [('cached_overview','skyblock_profile_context',{'mode':'cached'},30),('cached_equipment','skyblock_profile_section',{'mode':'cached','section':'equipment','offset':0,'limit':20},24)]:
                phases[name]=[call(tool,extra) for _ in range(n)]
            cpu1,peak=resource(process.pid)
            return {'runtime':kind,'logging':logging,'startupMs':startup,'firstSnapshotMs':cold,'cpuSeconds':None if cpu0 is None or cpu1 is None else cpu1-cpu0,'peakRssMiB':None if peak is None else peak/1048576,'samples':phases}
        finally:
            process.terminate()
            try:process.wait(timeout=5)
            except subprocess.TimeoutExpired:process.kill();process.wait()

def summary(rows):
    result={}
    for kind in ['node','rust']:
        group=[r for r in rows if r['runtime']==kind]
        values={k:statistics.median(r[k] for r in group if r[k] is not None) for k in ['startupMs','firstSnapshotMs','cpuSeconds','peakRssMiB'] if any(r[k] is not None for r in group)}
        for phase in ['cached_overview','cached_equipment']:
            samples=sorted(x for r in group for x in r['samples'][phase]);values[phase]={'count':len(samples),'p50Ms':statistics.median(samples),'p95Ms':samples[math.ceil(len(samples)*.95)-1]}
        result[kind]=values
    return result

def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--node',required=True);p.add_argument('--node-main',required=True);p.add_argument('--rust',required=True);p.add_argument('--rounds',type=int,default=4);p.add_argument('--output',type=pathlib.Path,required=True);a=p.parse_args()
    if not 2<=a.rounds<=10:p.error('--rounds must be between 2 and 10')
    rows=[]
    for logging in [False,True]:
        for i in range(a.rounds):
            for kind in (['node','rust'] if i%2==0 else ['rust','node']):rows.append(run(kind,a,logging))
    output={'method':'Counterbalanced fresh processes; invented demo data; 30 cached overviews and 24 equipment reads per process; localhost HTTP; no upstream requests','rounds':a.rounds,'apiRequests':0,'loggingDisabled':summary([r for r in rows if not r['logging']]),'loggingEnabled':summary([r for r in rows if r['logging']]),'runs':rows}
    a.output.write_text(json.dumps(output,indent=2)+'\n');a.output.chmod(0o600);print(json.dumps({k:v for k,v in output.items() if k!='runs'},indent=2))
if __name__=='__main__':main()
