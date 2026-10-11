"""Reject invalid mixin package ownership before producing a mod JAR."""
import json
from pathlib import Path

def validate(root):
    root = Path(root)
    resources = root / 'src/main/resources'
    metadata = json.loads((resources / 'fabric.mod.json').read_text())
    for name in metadata.get('mixins', []):
        config = json.loads((resources / name).read_text())
        package = config['package']
        prefix = package + '.'
        mixins = {prefix + value for key in ('mixins', 'client', 'server') for value in config.get(key, [])}
        for entries in metadata.get('entrypoints', {}).values():
            for entry in entries:
                value = entry if isinstance(entry, str) else entry['value']
                if value.startswith(prefix):
                    raise ValueError('Client entrypoint is inside a reserved mixin package')
        for source in (root / 'src/main/java').rglob('*.java'):
            qualified = '.'.join(source.relative_to(root / 'src/main/java').with_suffix('').parts)
            if qualified.startswith(prefix) and qualified not in mixins:
                raise ValueError('Ordinary class is inside a reserved mixin package: ' + qualified)
        for qualified in mixins:
            if not (root / 'src/main/java' / (qualified.replace('.', '/') + '.java')).is_file():
                raise ValueError('Configured mixin source is missing: ' + qualified)
