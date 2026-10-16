#!/usr/bin/env python3
"""Build against an installed Prism/Fabric 26.2 instance; never downloads or runs a game."""
from pathlib import Path
from validate import validate
import argparse,hashlib,json,os,subprocess,zipfile,shutil

root=Path(__file__).resolve().parent
p=argparse.ArgumentParser();p.add_argument('--prism-root',type=Path,required=True);p.add_argument('--instance',required=True);a=p.parse_args()
validate(root)
prism=a.prism_root.resolve();instance=prism/'instances'/a.instance
components=json.loads((instance/'mmc-pack.json').read_text())['components']
assert any(c['uid']=='net.minecraft' and c['version']=='26.2' for c in components),'Minecraft 26.2 is required'
mods=instance/'minecraft/mods';fabric=mods/'fabric-api-0.161.0+26.2.jar'
build=root/'build';classes=build/'classes';dependencies=build/'dependencies'
if classes.exists():shutil.rmtree(classes)
classes.mkdir(parents=True);dependencies.mkdir(parents=True,exist_ok=True)
with zipfile.ZipFile(fabric) as z:
 for name in z.namelist():
  if name.startswith('META-INF/jars/') and name.endswith('.jar') and any(x in name for x in ['fabric-lifecycle-events-v1-','fabric-api-base-','fabric-message-api-v1-']):
   target=dependencies/Path(name).name;target.write_bytes(z.read(name))
libraries=[p for p in (prism/'libraries').rglob('*.jar') if '/minecraft/' not in str(p) and '/intermediary/' not in str(p) and not p.name.endswith('-sources.jar')]
libraries=[prism/'libraries/com/mojang/minecraft/26.2/minecraft-26.2-client.jar',*libraries,*dependencies.glob('*.jar')]
sources=sorted((root/'src/main/java').rglob('*.java'))
subprocess.run(['javac','-encoding','UTF-8','--release','25','-proc:none','-classpath',os.pathsep.join(map(str,libraries)),'-d',str(classes),*map(str,sources)],check=True)
test=root/'src/test/java/dev/blockcontext/client/MenuParserTest.java'
subprocess.run(['javac','-encoding','UTF-8','--release','25','-classpath',str(classes),'-d',str(classes),str(test)],check=True)
subprocess.run(['java','-ea','-classpath',str(classes),'dev.blockcontext.client.MenuParserTest'],check=True)
output=build/'BlockContext-Collector-0.3.7-mc26.2.jar'
with zipfile.ZipFile(output,'w',zipfile.ZIP_DEFLATED) as z:
 for base in [classes,root/'src/main/resources']:
  for f in sorted(base.rglob('*')):
   if f.is_file() and not f.name.endswith('Test.class'):z.write(f,f.relative_to(base).as_posix())
 (root.parent/'LICENSE').exists() and z.write(root.parent/'LICENSE','LICENSE')
manifest={'file':output.name,'sha256':hashlib.sha256(output.read_bytes()).hexdigest(),'java':25,'minecraft':'26.2','fabricLoader':'0.19.5','fabricApi':'0.161.0+26.2','sources':[str(s.relative_to(root)) for s in sources],'runtimeValidated':False}
(build/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');print(json.dumps(manifest))
