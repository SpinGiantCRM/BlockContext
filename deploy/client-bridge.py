#!/usr/bin/env python3
"""Send changed private snapshots to the operator's verified SSH server. No API calls."""
import argparse, hashlib, json, os, re, stat, subprocess, time
from pathlib import Path
from client_snapshot import LIMIT, project

def private_read(path, limit):
    fd=os.open(path, os.O_RDONLY | getattr(os,'O_NOFOLLOW',0))
    try:
        info=os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or info.st_size>limit or (os.name!='nt' and info.st_mode & 0o077): raise ValueError('not a bounded private file')
        with os.fdopen(fd,'rb',closefd=False) as f: return f.read(limit+1)
    finally: os.close(fd)

def main():
    p=argparse.ArgumentParser();p.add_argument('--config',type=Path,required=True);a=p.parse_args()
    cfg=json.loads(private_read(a.config,8192));host=cfg['sshHost'];owner=cfg['ownerUuid']
    if not re.fullmatch(r'[A-Za-z0-9_.-]{1,64}@[A-Za-z0-9.-]{1,253}',host): raise ValueError('invalid SSH destination')
    identity=Path(cfg['sshIdentity']).resolve(strict=True);source=Path(cfg['source'])
    if not source.is_absolute(): raise ValueError('absolute snapshot path required')
    # Verify identity permissions without reading or logging its contents.
    info=identity.stat()
    if not stat.S_ISREG(info.st_mode) or (os.name!='nt' and info.st_mode & 0o077): raise ValueError('private SSH identity required')
    # Explicit identity/destination and default known-hosts file; ignore unrelated proxy configuration.
    command=['ssh','-F',os.devnull,'-o','BatchMode=yes','-o','StrictHostKeyChecking=yes','-o','ConnectTimeout=10','-o','ServerAliveInterval=5','-o','ServerAliveCountMax=2','-i',str(identity),host,'sudo /usr/local/bin/blockcontext-import-client-snapshot']
    control=Path(os.environ.get('XDG_RUNTIME_DIR', '/tmp'))/('blockcontext-ssh-'+str(os.getuid()))
    control.mkdir(mode=0o700,exist_ok=True)
    if control.is_symlink() or control.stat().st_uid!=os.getuid() or control.stat().st_mode & 0o077: raise ValueError('private SSH control directory required')
    command[1:1]=['-o','ControlMaster=auto','-o','ControlPersist=60','-o','ControlPath='+str(control/'bridge')]
    previous=None;last_status=None
    while True:
        try:
            blob=project(private_read(source,LIMIT),owner);digest=hashlib.sha256(blob).digest()
            if digest!=previous:
                run=subprocess.run(command,input=blob,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=25)
                if run.returncode: raise ValueError('upload failed')
                previous=digest
            status='ready'
        except FileNotFoundError: status='waiting_for_snapshot'
        except (ValueError,TypeError,KeyError,OSError,subprocess.TimeoutExpired,json.JSONDecodeError): status='snapshot_or_upload_unavailable'
        if status!=last_status: print('BlockContext client bridge: '+status,flush=True);last_status=status
        time.sleep(2)

if __name__=='__main__': main()
