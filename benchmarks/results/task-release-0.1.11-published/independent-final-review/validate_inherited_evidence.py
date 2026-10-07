from pathlib import Path
import hashlib,json,re,subprocess,sys
REPO=Path('/tmp/nagi-release-0.1.11')
OUTPUT=Path('/tmp/nagi-critical-release-review/final-integrated-release')
REF=sys.argv[1] if len(sys.argv)>1 else 'c4ddc23a905c0f0e3642a6c820c6b5d5e652b241'
assert re.fullmatch(r'[0-9a-f]{40}',REF)
def source(path):
    return subprocess.run(['git','-C',str(REPO),'show',REF+':'+path],capture_output=True,check=True).stdout
provenance=json.loads(source('benchmarks/results/task-release-0.1.11/provenance.json'))
source_checks=[]
artifact_checks=[]
for path,expected in provenance['candidate_source_sha256'].items():
    actual=hashlib.sha256(source(path)).hexdigest()
    source_checks.append({'path':path,'matches':actual==expected,'expected':expected,'actual':actual})
for path,expected in provenance['artifact_sha256'].items():
    if not path.startswith('benchmarks/'):
        path='benchmarks/results/task-release-0.1.11/'+path
    actual=hashlib.sha256(source(path)).hexdigest()
    artifact_checks.append({'path':path,'matches':actual==expected,'expected':expected,'actual':actual})
folder='benchmarks/results/task-release-0.1.11/conditional-fix-final/ci/'
validated=json.loads(source(folder+'validated.json'))
platform_checks={}
for platform,reported in validated['platforms'].items():
    raw=source(folder+platform+'.log')
    text=raw.decode()
    actual=hashlib.sha256(raw).hexdigest()
    oracles=['Task contracts: 148/148 matched','test task_conditional_consumption::always_evaluated_receipts_and_lazy_values_run_in_three_sources ... ok','test task_service_s2::typed_monitor_preserves_service_errors_shutdown_and_parent_drop ... ok','Verified Task handles: extracted compiler/runtime']
    assert actual==reported['log_sha256'],platform
    assert all(marker in text for marker in oracles),platform
    platform_checks[platform]={'raw_log_sha256':actual,'reported_sha256_matches':True,'dedicated_conditional_and_service_oracles_present':True,'contracts148_and_extracted_archive_gate_present':True,'test_execution':'inherited CI; no local tests run'}
original=Path('/tmp/nagi-critical-release-review/post-fix')
copied='benchmarks/results/task-release-0.1.11/conditional-fix-final/independent-post-fix/'
review_copies=[]
for file in ['final-review.txt','findings.json','final-provenance.json','checker-results.json','boundary-control-results.json','conditional-native-independent.log']:
    assert source(copied+file)==(original/file).read_bytes(),file
    review_copies.append(file)
report={'examined_sha':REF,'source_hashes_checked':len(source_checks),'artifact_hashes_checked':len(artifact_checks),'source_mismatches':[v for v in source_checks if not v['matches']],'artifact_mismatches':[v for v in artifact_checks if not v['matches']],'platform_raw_log_evidence':platform_checks,'own_historic_review_copies_byte_identical':review_copies,'remote_api_status_not_independently_read':True,'source_checks':source_checks,'artifact_checks':artifact_checks}
(OUTPUT/('inherited-evidence-validation-'+REF[:8]+'.json')).write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
print(json.dumps({k:v for k,v in report.items() if k not in ['source_checks','artifact_checks']},ensure_ascii=False,indent=2))
