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
    def test_commission_counter_projection_and_conflicts(self):
        r=fixture();r['collectorVersion']='0.3.1';obs=r['profiles'][0]['observations']
        obs['commissions']=dict(observedAt=NOW,readFrom='opened_menu',values=dict(remainingBonusCommissions=0,nextResetSecondsAtObservation=100,private='discard'))
        result=json.loads(project(json.dumps(r).encode(),OWNER,NOW))['profiles'][0]['observations']['commissions']['values']
        self.assertEqual(result,dict(remainingBonusCommissions=0,nextResetSecondsAtObservation=100))
        obs['commissions']['values']['dailyBonusAvailable']=True
        with self.assertRaises(ValueError):project(json.dumps(r).encode(),OWNER,NOW)

    def test_collector_versions(self):
        for version in ('0.1.0', '0.1.1','0.2.0','0.2.1','0.2.2','0.3.0', '0.3.1'):
            r=fixture();r['collectorVersion']=version
            self.assertEqual(json.loads(project(json.dumps(r).encode(),OWNER,NOW))['collectorVersion'],version)
        r=fixture();r['collectorVersion']='9.9.9'
        with self.assertRaises(ValueError):project(json.dumps(r).encode(),OWNER,NOW)
    def test_projection_and_zero(self):
        blob=project(json.dumps(fixture()).encode(),OWNER,NOW)
        self.assertNotIn(b'private',blob);self.assertNotIn(b'discard',blob)
        self.assertEqual(json.loads(blob)['profiles'][0]['observations']['bank:personalBankCoins']['values']['personalBankCoins'],0)
    def test_passive_live_state_changes_and_attributes(self):
        r=fixture();r['collectorVersion']='0.3.1';r['observedNow']=['inventory'];r['exportIntervalMs']=1000;p=r['profiles'][0];obs=p['observations']
        weapon=obs['huntaxe']['values']['weapon'];weapon['structuredAttributes']=dict(fields=[dict(path='upgrade_level',value=5,semanticStatus='reported_attribute')],omitted=0,truncated=False,complete=False)
        obs['inventory']=dict(observedAt=NOW,readFrom='player_inventory',values=dict(items=[weapon],observedSlots=41,completeStorage=False))
        p['changes']=[dict(path='inventory.items.0.count',previousValue=1,value=2,previousObservedAt=NOW-1000,observedAt=NOW,kind='observed_value_change')]
        out=json.loads(project(json.dumps(r).encode(),OWNER,NOW));self.assertEqual(out['observedNow'],['inventory']);self.assertEqual(out['profiles'][0]['changes'][0]['value'],2)
        p['changes'][0]['path']='inventory.secret'
        with self.assertRaises(ValueError):project(json.dumps(r).encode(),OWNER,NOW)
    def test_other_owner(self):
        with self.assertRaises(ValueError):project(json.dumps(fixture()).encode(),'b'*32,NOW)
    def test_bounds(self):
        for change in [lambda r:r.update(exportedAt=NOW+5001),lambda r:r.update(exportedAt=NOW-86400_001),lambda r:r.update(activeProfile='unknown'),lambda r:r.update(profiles=r['profiles']*9),lambda r:r['profiles'][0]['observations']['huntaxe']['values']['weapon'].update(slot=54),lambda r:r['profiles'][0]['observations']['bank:personalBankCoins']['values'].update(personalBankCoins=float('nan'))]:
            r=fixture();change(r)
            with self.assertRaises((ValueError,TypeError)):project(json.dumps(r).encode(),OWNER,NOW)
        with self.assertRaises(ValueError):project(b' '* (LIMIT+1),OWNER,NOW)
    def test_new_inventory_and_menu_projection(self):
        r=fixture();r['collectorVersion']='0.2.0';obs=r['profiles'][0]['observations'];weapon=obs['huntaxe']['values']['weapon']
        weapon.update(instanceId='f'*64,attributes={'warrior':2},abilityScrolls=['IMPLOSION_SCROLL'],gems={'RUBY_0':{'quality':'PERFECT','type':'RUBY','secret':'private'}},storedWeapon=dict(status='partial',displayName='Fabled Hyper Cleaver',stars=5,completeWeaponDetails=False))
        obs['inventory']=dict(observedAt=NOW,readFrom='player_inventory',values=dict(items=[weapon],observedSlots=41,completeStorage=False))
        obs['mining']=dict(observedAt=NOW,readFrom='opened_menu',values=dict(facts=[dict(slot=1,displayName='Mining Speed',fields=[dict(label='Level',value=20,maximum=50,unit=None,secret='private')])],observedSlots=54,completeSystem=False))
        result=project(json.dumps(r).encode(),OWNER,NOW);self.assertNotIn(b'private',result)
        out=json.loads(result)['profiles'][0]['observations'];self.assertEqual(out['inventory']['values']['items'][0]['attributes'],{'warrior':2});self.assertEqual(out['mining']['values']['facts'][0]['fields'][0]['maximum'],50)
        obs['mining']['values']['completeSystem']=True
        with self.assertRaises(ValueError):project(json.dumps(r).encode(),OWNER,NOW)
    def test_old_fields_expire_without_history(self):
        r=fixture();r['profiles'][0]['observations']['huntaxe']['observedAt']=NOW-86400_001
        self.assertNotIn('huntaxe',json.loads(project(json.dumps(r).encode(),OWNER,NOW))['profiles'][0]['observations'])

if __name__=='__main__':unittest.main()
