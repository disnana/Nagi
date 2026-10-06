import json
import re
from pathlib import Path

artifact = Path(__file__).resolve().parent
names = [
    'canonical_task_metadata_survives_saved_low_and_user_task_names',
    'raw_check_registers_contextual_task_and_preserves_obligation',
    'native_business_fault_cleanup_and_branch_contracts',
    'bounded_generated_task_paths_reach_native',
    'positive_contracts_build_and_run_high_saved_and_handwritten_low',
]
observations = {}
for platform in ['linux-x86_64', 'windows-x86_64', 'macos-arm64', 'macos-x86_64']:
    text = (artifact / (platform + '.log')).read_text(encoding='utf-8-sig')
    text = re.sub(r'\x1b\[[0-9;]*m', '', text)
    text = re.sub(r'^\d{4}-\d\d-\d\dT\S+Z ', '', text, flags=re.M)
    blocks = re.findall(r'Running tests[\\/]task_handles\.rs[^\n]*\n(.*?)(?=\n\s*Running |\n\s*Doc-tests |\n##\[group\]|\Z)', text, flags=re.S)
    native = [b for b in blocks if 'test result: ok. 5 passed; 0 failed; 0 ignored;' in b and all('test '+n+' ... ok' in b for n in names)]
    public = [b for b in blocks if 'test result: ok. 3 passed; 0 failed; 0 ignored;' in b]
    row = {
        'native_tests': 5 if native else 0,
        'public_api_tests': 3 if public else 0,
        'runtime_oracles': 17 if re.search(r'^test result: ok\. 17 passed; 0 failed; 0 ignored;', text, re.M) else 0,
        'runtime_doc_tests': 9 if re.search(r'Doc-tests nagi_runtime\s+running 9 tests.*?test result: ok\. 9 passed; 0 failed; 0 ignored;',text,re.S) else 0,
        'strict_contracts': 90 if re.search(r'^Task contracts: 90/90 matched;', text, re.M) else 0,
    }
    assert row == dict(native_tests=5, public_api_tests=3, runtime_oracles=17, runtime_doc_tests=9, strict_contracts=90), (platform, row)
    observations[platform] = row
text = (artifact/'linux-full.log').read_text(encoding='utf-8-sig')
text = re.sub(r'\x1b\[[0-9;]*m', '', text)
text = re.sub(r'^\d{4}-\d\d-\d\dT\S+Z ', '', text, flags=re.M)
block = re.search(r'^##\[group\]Run cargo test --locked\n(.*?)(?=^##\[group\]|\Z)', text, re.M|re.S)
assert block
rows = re.findall(r'^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', block.group(1), re.M)
workspace = {'result_blocks':len(rows),'passed':sum(int(r[0]) for r in rows),'failed':sum(int(r[1]) for r in rows),'ignored':sum(int(r[2]) for r in rows),'includes':'three image_child subprocess reruns; cost oracle is ignored here and explicitly measured locally'}
assert (workspace['result_blocks'],workspace['passed'],workspace['failed'],workspace['ignored']) == (95,933,0,1)
record = {'source_head':'34ac4d585084877372965e3d58ed5c2002604529','tree':'781e031eafbdc0efa00038007d41cf16dc538862','checks_run':37489343115,'website_run':37489342523,'task_observations':observations,'linux_workspace':workspace,'scope':'Counts are verified in decoded job logs; accepted checker contracts are not a native test count.'}
(artifact/'task-observations.json').write_text(json.dumps(record,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(record,ensure_ascii=False))
