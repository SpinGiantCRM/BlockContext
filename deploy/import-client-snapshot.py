#!/usr/bin/env python3
"""Root-owned SSH import helper. Fixed destination; no user-provided commands or paths."""
import os, pwd, sys, tempfile
from pathlib import Path
from client_snapshot import LIMIT, project

def main():
    temp=None
    try:
        owner=Path('/etc/blockcontext/client-owner').read_text().strip()
        blob=project(sys.stdin.buffer.read(LIMIT+1),owner)
        dest=Path('/var/lib/blockcontext/client/latest.json');directory=dest.parent
        if directory.is_symlink() or dest.is_symlink(): raise ValueError('unsafe destination')
        directory.mkdir(mode=0o700,exist_ok=True);os.chmod(directory,0o700)
        account=pwd.getpwnam('blockcontext');os.chown(directory,account.pw_uid,account.pw_gid)
        fd,temp=tempfile.mkstemp(prefix='.snapshot-',dir=directory)
        with os.fdopen(fd,'wb') as f: f.write(blob);f.flush();os.fsync(f.fileno());os.fchmod(f.fileno(),0o600);os.fchown(f.fileno(),account.pw_uid,account.pw_gid)
        os.replace(temp,dest);temp=None
    except (ValueError,KeyError,TypeError,OSError):
        print('Snapshot import rejected.',file=sys.stderr);return 1
    finally:
        if temp is not None: os.unlink(temp)
    return 0

if __name__=='__main__': sys.exit(main())
