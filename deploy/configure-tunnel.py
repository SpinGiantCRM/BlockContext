#!/usr/bin/env python3
"""Privately transfer a runtime tunnel credential and validate it before starting."""
import argparse
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys

parser = argparse.ArgumentParser()
parser.add_argument('--host', required=True)
parser.add_argument('--identity', required=True)
parser.add_argument('--env', default=str(Path(__file__).resolve().parents[1] / '.env.tunnel'))
args = parser.parse_args()
private = Path(args.env)
if private.stat().st_mode & 0o077:
    sys.exit('The private tunnel file must have 600 permissions.')
values = {}
allowed = {'CONTROL_PLANE_TUNNEL_ID', 'CONTROL_PLANE_API_KEY', 'CONTROL_PLANE_ORGANIZATION_ID'}
for line in private.read_text().splitlines():
    if not line.strip() or line.lstrip().startswith('#'):
        continue
    name, separator, value = line.partition('=')
    if separator and name.strip() in allowed:
        value = value.strip()
        if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
            value = value[1:-1]
        values[name.strip()] = value
if not re.fullmatch(r'tunnel_[A-Za-z0-9_-]{8,}', values.get('CONTROL_PLANE_TUNNEL_ID', '')):
    sys.exit('Fill CONTROL_PLANE_TUNNEL_ID in the private file first.')
key = values.get('CONTROL_PLANE_API_KEY', '')
if not re.fullmatch(r'sk-[A-Za-z0-9_-]{16,}', key) or key.startswith('sk-admin'):
    sys.exit('Fill CONTROL_PLANE_API_KEY with a runtime API key, never an admin key.')
org = values.get('CONTROL_PLANE_ORGANIZATION_ID', '')
if org and not re.fullmatch(r'org-[A-Za-z0-9_-]+', org):
    sys.exit('Use the Platform organization ID, or leave its optional field blank.')

remote = r'''
import json, os, subprocess, sys
from pathlib import Path
values = json.load(sys.stdin)
private = Path('/etc/blockcontext/tunnel.env')
pending = private.with_suffix('.env.pending')
fd = os.open(pending, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
os.fchmod(fd, 0o600)
with os.fdopen(fd, 'w') as file:
    file.write(''.join(name + '=' + value + '\n' for name, value in values.items() if value))
os.replace(pending, private)
env = dict(os.environ, **{k:v for k,v in values.items() if v})
command = ['/usr/sbin/runuser', '-u', 'blockcontext-tunnel', '--', '/opt/blockcontext/tunnel/tunnel-client', 'doctor', '--json', '--mcp.server-url', 'http://127.0.0.1:3008/mcp', '--health.listen-addr', '127.0.0.1:3009']
try:
    checked = subprocess.run(command, env=env, capture_output=True, text=True, timeout=60)
except subprocess.TimeoutExpired:
    sys.exit('Tunnel checks timed out; service has not been started.')
if checked.returncode:
    # Keep complete diagnostics privately on the host, rather than copying headers/logs to chat.
    diagnostic = Path('/etc/blockcontext/tunnel-doctor-private.log')
    descriptor = os.open(diagnostic, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    os.fchmod(descriptor, 0o600)
    with os.fdopen(descriptor, 'w') as file:
        file.write(checked.stdout + checked.stderr)
    sys.exit('Tunnel validation failed. Private host diagnostics are saved; service has not been started.')
print('Tunnel credentials and target passed the official client checks.')
subprocess.run(['systemctl', 'enable', 'blockcontext-tunnel.service'], check=True)
subprocess.run(['systemctl', 'restart', 'blockcontext-tunnel.service'], check=True)
'''
ssh = ['ssh', '-o', 'BatchMode=yes', '-o', 'StrictHostKeyChecking=yes', '-o', 'ConnectTimeout=10', '-i', args.identity, args.host, 'sudo python3 -c ' + shlex.quote(remote)]
result = subprocess.run(ssh, input=json.dumps(values), text=True, timeout=90)
sys.exit(result.returncode)
