import json, sys, tempfile, unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'client-mod'))
from validate import validate

class MixinOwnership(unittest.TestCase):
    def test_current_configuration(self):
        validate(Path(__file__).resolve().parents[1] / 'client-mod')

    def test_rejects_original_crash_layout(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            resources = root / 'src/main/resources'; resources.mkdir(parents=True)
            (resources / 'fabric.mod.json').write_text(json.dumps({'mixins':['mixins.json'], 'entrypoints':{'client':['dev.blockcontext.client.BlockContextClient']}}))
            config = {'package':'dev.blockcontext.client', 'client':['TabOverlayAccess']}
            (resources / 'mixins.json').write_text(json.dumps(config))
            with self.assertRaisesRegex(ValueError, 'entrypoint'):
                validate(root)
            # Moving only the entrypoint leaves helpers equally vulnerable.
            (resources / 'fabric.mod.json').write_text(json.dumps({'mixins':['mixins.json'], 'entrypoints':{}}))
            source = root / 'src/main/java/dev/blockcontext/client/MenuParser.java'
            source.parent.mkdir(parents=True); source.write_text('package dev.blockcontext.client;')
            with self.assertRaisesRegex(ValueError, 'Ordinary class'):
                validate(root)
