"""Cross-runtime boundary checks for experiment observations/checkpoints."""
import copy
import json
from pathlib import Path
import sys
import unittest
sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from client_snapshot import project
OWNER='a'*32
NOW=1_800_000_000_000

def fixture():
    ledger=dict(ledgerCreatedAt=NOW-1000,persistentObservedChargeDecreases=1,persistentObservedChargeIncreases=0,observationGaps=1,restoredFromDisk=True,persistenceAvailable=True,completeDailyCount=False,lastObservedCharges=0,lastChargesObservedAt=NOW)
    records={'experimentTracking':dict(readFrom='collector_ledger',observedAt=NOW,values=ledger),'experimentTable':dict(readFrom='opened_menu',observedAt=NOW,values=dict(availableCharges=0,maximumCharges=3,paidResetsUsedToday=0,maximumPaidResetsToday=3,paidResetsRemainingToday=3,nextChargeSecondsAtObservation=82800,private='PRIVATE_SENTINEL'))}
    return dict(schemaVersion=1,collectorVersion='0.3.7',exportedAt=NOW,activeProfile=OWNER+':apple',profiles=[dict(uuid=OWNER,playerName='DemoPlayer',profileName='Apple',observations=records)])
class Experiments(unittest.TestCase):
    def test_scalar_projection_and_old_checkpoint_retention(self):
        value=fixture();out=project(json.dumps(value).encode(),OWNER,NOW)
        self.assertNotIn(b'PRIVATE_SENTINEL',out)
        rows=json.loads(out)['profiles'][0]['observations']
        self.assertEqual(rows['experimentTable']['values']['availableCharges'],0)
        old=json.loads(project(json.dumps(value).encode(),OWNER,NOW+2*86400_000))['profiles'][0]['observations']
        self.assertEqual(set(old),{'experimentTracking'})
    def test_rejects_wrong_sources_future_counts_and_false_certainty(self):
        for key,field,value in [('experimentTable','readFrom','collector_ledger'),('experimentTracking','readFrom','opened_menu')]:
            raw=fixture();raw['profiles'][0]['observations'][key][field]=value
            with self.assertRaises(ValueError):project(json.dumps(raw).encode(),OWNER,NOW)
        for field,value in [('completeDailyCount',True),('lastChargesObservedAt',NOW+1),('persistentObservedChargeDecreases',-1)]:
            raw=fixture();raw['profiles'][0]['observations']['experimentTracking']['values'][field]=value
            with self.assertRaises(ValueError):project(json.dumps(raw).encode(),OWNER,NOW)
        for value in [{},{'availableCharges':True},{'availableCharges':1.5},{'availableCharges':4,'maximumCharges':3}]:
            raw=fixture();raw['profiles'][0]['observations']['experimentTable']['values']=value
            with self.assertRaises(ValueError):project(json.dumps(raw).encode(),OWNER,NOW)
