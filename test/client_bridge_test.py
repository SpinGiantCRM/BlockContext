import copy, json, sys, unittest
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'deploy'))
from client_snapshot import project, LIMIT
OWNER='a'*32
NOW=1_800_000_000_000

def fixture():
    item=dict(slot=22,id='HYPER_CLEAVER',count=1,reforge='fabled',enchantments={'sharpness':6},lore='private',uuid='discard')
    return dict(schemaVersion=1,collectorVersion='0.1.0',exportedAt=NOW,activeProfile=OWNER+':strawberry',profiles=[dict(uuid=OWNER,playerName='Player',profileName='Strawberry',observations={'huntaxe':dict(observedAt=NOW,readFrom='opened_menu',values={'weapon':item}),'bank:personalBankCoins':dict(observedAt=NOW,readFrom='opened_menu',values={'personalBankCoins':0}), 'unrecognized':dict(observedAt=NOW,readFrom='opened_menu',values={'chat':'private'})})])

class Projection(unittest.TestCase):
    def test_projection_and_zero(self):
        blob=project(json.dumps(fixture()).encode(),OWNER,NOW)
        self.assertNotIn(b'private',blob);self.assertNotIn(b'discard',blob)
        self.assertEqual(json.loads(blob)['profiles'][0]['observations']['bank:personalBankCoins']['values']['personalBankCoins'],0)
    def test_other_owner(self):
        with self.assertRaises(ValueError):project(json.dumps(fixture()).encode(),'b'*32,NOW)
    def test_bounds(self):
        for change in [lambda r:r.update(exportedAt=NOW+5001),lambda r:r.update(exportedAt=NOW-86400_001),lambda r:r.update(activeProfile='unknown'),lambda r:r.update(profiles=r['profiles']*9),lambda r:r['profiles'][0]['observations']['huntaxe']['values']['weapon'].update(slot=54),lambda r:r['profiles'][0]['observations']['bank:personalBankCoins']['values'].update(personalBankCoins=float('nan'))]:
            r=fixture();change(r)
            with self.assertRaises((ValueError,TypeError)):project(json.dumps(r).encode(),OWNER,NOW)
        with self.assertRaises(ValueError):project(b' '* (LIMIT+1),OWNER,NOW)
    def test_old_fields_expire_without_history(self):
        r=fixture();r['profiles'][0]['observations']['huntaxe']['observedAt']=NOW-86400_001
        self.assertNotIn('huntaxe',json.loads(project(json.dumps(r).encode(),OWNER,NOW))['profiles'][0]['observations'])

if __name__=='__main__':unittest.main()
