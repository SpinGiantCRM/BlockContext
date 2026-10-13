import importlib.util,pathlib,tempfile,unittest,json,datetime,time
spec=importlib.util.spec_from_file_location('report',pathlib.Path(__file__).parents[1]/'logging-report.py');r=importlib.util.module_from_spec(spec);spec.loader.exec_module(r)
class ReportTests(unittest.TestCase):
 def test_separates_processes_and_handles_missing_native_counters(self):
  rows=[dict(event='resource',runtime='node',heapUsedBytes=100,rssBytes=200,uptimeSeconds=10,cpuUserMicroseconds=100000,cpuSystemMicroseconds=0),dict(event='resource',runtime='rust',runtimeVersion='0.11.1',processStartedAt='a',rssBytes=100,uptimeSeconds=20,cpuUserMicroseconds=900000,cpuSystemMicroseconds=0),dict(event='resource',runtime='rust',runtimeVersion='0.11.1',processStartedAt='b',rssBytes=None,uptimeSeconds=100,cpuUserMicroseconds=9999999,cpuSystemMicroseconds=0),dict(event='resource',runtime='rust',runtimeVersion='0.11.1',processStartedAt='b',rssBytes=None,uptimeSeconds=160,cpuUserMicroseconds=10099999,cpuSystemMicroseconds=0),dict(event='resource',runtime='rust',uptimeSeconds=200,cpuUserMicroseconds=None,cpuSystemMicroseconds=None)]
  x=r.summarize(r.classify(rows));self.assertEqual(x['resources']['cpuIntervals'],1);self.assertEqual(x['resources']['rssSamples'],2)
 def test_legacy_node_and_new_rust_keep_cache_metrics_distinct(self):
  rows=[dict(event='resource',heapUsedBytes=100),dict(event='tool',tool='skyblock_profile_context',durationMs=30,cache='miss'),dict(event='lifecycle',state='started'),dict(event='resource',runtime='rust',runtimeVersion='0.11.1'),dict(event='tool',tool='skyblock_profile_context',durationMs=2,cache='fresh')]
  x=r.classify(rows);self.assertEqual(x[1]['_runtime'],'node');self.assertEqual(x[4]['_runtime'],'rust');stats=r.summarize(x)['tools']['skyblock_profile_context']['byCache'];self.assertEqual(stats['fresh']['p50Ms'],2);self.assertEqual(stats['miss']['p50Ms'],30)
 def test_private_or_malformed_labels_are_not_emitted(self):
  x=r.summarize(r.classify([dict(event='tool',tool='private-player-identity',durationMs=None,workload='private-token',cache='private-query'),dict(event='stage',stage='private-bank-value',durationMs=1)]));s=json.dumps(x);self.assertNotIn('private-',s);self.assertEqual(x['tools']['unknown']['count'],0)
 def test_bounded_stage_samples_preserve_partial_coverage(self):
  rows=r.classify([dict(event='stage',stage='cache_read',durationsMs=[1,2,3],calls=9,omittedSamples=6)])
  x=r.summarize(rows)['stages']['cache_read'];self.assertEqual(x['count'],3);self.assertEqual(x['totalCalls'],9);self.assertEqual(x['omittedSamples'],6);self.assertEqual(x['coverage'],'sample_limited')
 def test_safe_counters_and_native_gauges(self):
  rows=r.classify([dict(event='counters',runtime='rust',values={'cache_hits':5,'private-player-name':9,'cache_misses':None}),dict(event='resource',runtime='rust',peakRssBytes=4096,threads=3,openFileDescriptors=7,readBytes=None,writeBytes=20)])
  x=r.summarize(rows);self.assertEqual(x['counters'],{'cache_hits':5});self.assertEqual(x['resources']['threads']['maximum'],3);self.assertIsNone(x['resources']['readBytes']['maximum']);self.assertNotIn('private-player-name',json.dumps(x))
if __name__=='__main__':unittest.main()
