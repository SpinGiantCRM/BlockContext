#!/usr/bin/env python3
"""Test real Fabric loading from installed libraries; no game launch or account access."""
import argparse, json, os, platform, shutil, subprocess, tempfile
from pathlib import Path
from urllib.parse import urlparse, unquote
from validate import validate

root = Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
parser.add_argument('--prism-root', type=Path, required=True)
parser.add_argument('--instance', required=True)
parser.add_argument('--jar', type=Path, default=root/'build/BlockContext-Collector-0.2.0-mc26.2.jar')
args = parser.parse_args()
validate(root)
prism = args.prism_root.resolve()
system = {'Linux':'linux', 'Windows':'windows', 'Darwin':'osx'}[platform.system()]
libraries = []
for package in ('net.minecraft/26.2', 'net.fabricmc.fabric-loader/0.19.5', 'org.lwjgl3/3.4.1'):
    metadata = json.loads((prism/'meta'/ (package+'.json')).read_text())
    entries = metadata['libraries'] + ([metadata['mainJar']] if 'mainJar' in metadata else [])
    for entry in entries:
        rules = entry.get('rules')
        if rules and not any(r.get('action')=='allow' and r.get('os',{}).get('name',system)==system for r in rules): continue
        parts = entry['name'].split(':'); group, artifact, version = parts[:3]
        filename = artifact+'-'+version+('-'+parts[3] if len(parts)>3 else '')+'.jar'
        library = prism/'libraries'/group.replace('.', '/')/artifact/version/filename
        if not library.is_file():
            url = entry.get('downloads',{}).get('artifact',{}).get('url','')
            path = unquote(urlparse(url).path).lstrip('/')
            library = prism/'libraries'/path
        if not library.is_file(): raise RuntimeError('Missing installed library: '+entry['name'])
        libraries.append(library)

with tempfile.TemporaryDirectory(prefix='blockcontext-fabric-test-') as directory:
    game = Path(directory); (game/'mods').mkdir(); classes=game/'test-classes';classes.mkdir()
    shutil.copy2(args.jar,game/'mods'/args.jar.name)
    fabric=prism/'instances'/args.instance/'minecraft/mods/fabric-api-0.161.0+26.2.jar'
    shutil.copy2(fabric,game/'mods'/fabric.name)
    cp=os.pathsep.join(map(str,libraries))
    source=root/'src/test/java/dev/blockcontext/test/FabricStartupTest.java'
    subprocess.run(['javac','--release','25','-proc:none','-classpath',cp,'-d',str(classes),str(source)],check=True)
    subprocess.run(['java','-Xmx512m','-Djava.awt.headless=true','-Dfabric.gameVersion=26.2','-classpath',str(classes)+os.pathsep+cp,'dev.blockcontext.test.FabricStartupTest',str(game)],cwd=game,check=True,timeout=60)
