from pathlib import Path
import hashlib, json, re, subprocess, sys, tomllib, types
sys.dont_write_bytecode = True
REPO = Path('/tmp/nagi-release-0.1.11')
OUTPUT = Path('/tmp/nagi-critical-release-review/final-integrated-release')
FINAL = '64a7f3d5c5306abca579e5d95dd9990087747699'
INITIAL = '1062b6985195979a2afb4c16699607909ae3c44a'
FIXED = '97e62f82b1f67dbcea699071477cbeec7388a713'
OLD_RELEASE = '9cca4219b283d2ff11cd25ac4d4667a2192c85e0'
def git(*args):
    p = subprocess.run(['git', '-C', str(REPO), *args], capture_output=True, check=True)
    return p.stdout
def source(ref, path):
    return git('show', ref+':'+path)
def module(name, path):
    m = types.ModuleType(name)
    m.__file__ = str(REPO/path)
    sys.modules[name] = m
    exec(compile(source(FINAL, path), str(REPO/path), 'exec'), m.__dict__)
    return m
plan = module('plan', 'scripts/releases/plan.py')
notes = module('notes', 'scripts/releases/notes.py')
package = module('package', 'scripts/releases/package.py')
identity = {'sha':git('rev-parse', 'HEAD').decode().strip(), 'tree':git('rev-parse', 'HEAD^{tree}').decode().strip(), 'status':git('status', '--porcelain=v1').decode()}
assert identity == {'sha': FINAL, 'tree':'13b41a034bf463a9426c53327b42c4184a53c7ed', 'status':''}, identity
protected = ['compiler', 'runtime', 'scripts', '.github', '.codex', 'AGENTS.md']
assert not git('diff','--name-only',FIXED,FINAL,'--',*protected)
assert git('diff','--name-only',INITIAL,FINAL).decode().splitlines() == ['benchmarks/results/task-release-0.1.11/provenance.json', 'docs/task-handles.md']
delta = git('diff','--no-ext-diff',INITIAL,FINAL).decode()
(OUTPUT/'final-corrections.diff').write_text(delta)
ja = source(FINAL,'docs/task-handles.md').decode()
assert '短絡`and`/`or`の右辺は評価が省略されることがあり、値が存在する場合の`env` fallbackは評価されません。' in ja
release_plan = plan.plan(FIXED, FINAL)
assert release_plan == {'sha':FINAL, 'nagi_version':'0.1.11', 'release_nagi':'true', 'package_nagi':'true', 'vscode_version':'0.1.13', 'release_vscode':'false', 'package_vscode':'false'}, release_plan
entry = notes.changelog_entry(FINAL,'nagi','0.1.11')
assert len(re.findall(r'^- ',entry,re.M)) == 14
assert 'short-circuit operands and env fallbacks' in entry
assert '## Nagi 0.1.11 — 2026-10-07' in source(FINAL,'CHANGELOG.md').decode()
(OUTPUT/'extracted-release-notes.txt').write_text(entry+'\n')
workspace = tomllib.loads(source(FINAL,'Cargo.toml').decode())['workspace']['package']
manifest = package.runtime_manifest(source(FINAL,'runtime/Cargo.toml').decode(),workspace)
standalone = tomllib.loads(manifest.decode())
assert standalone['package']['version'] == '0.1.11'
assert all(not isinstance(standalone['package'][k],dict) for k in ['version','edition','license'])
(OUTPUT/'standalone-runtime.Cargo.toml').write_bytes(manifest)
platforms = ['linux-x86_64','macos-arm64','macos-x86_64','windows-x86_64']
archives = [package.archive_name('0.1.11',v) for v in platforms]
assert archives == ['nagi-0.1.11-linux-x86_64.tar.gz','nagi-0.1.11-macos-arm64.tar.gz','nagi-0.1.11-macos-x86_64.tar.gz','nagi-0.1.11-windows-x86_64.zip']
for exe in ['nagic','nagic.exe']:
    readme = package.readme('0.1.11',exe)
    assert readme.startswith(b'Nagi 0.1.11')
    (OUTPUT/('package-readme-'+exe+'.txt')).write_bytes(readme)
codes = []
for f in ['docs/migration-0.1.11.md','docs/en/migration-0.1.11.md']:
    extract = lambda data: re.search(r'```rust\n(.*?)\n```',data,re.S).group(1)
    current = extract(source(FINAL,f).decode())
    old = extract(source(OLD_RELEASE,f).decode())
    assert current == old
    codes.append(current)
assert codes[0] == codes[1]
report = {'identity':identity, 'fixed_main_tree':git('rev-parse',FIXED+'^{tree}').decode().strip(), 'checker_sha256':hashlib.sha256(source(FINAL,'compiler/src/check.rs')).hexdigest(), 'protected_source_identical_to_independently_reviewed_fix':True, 'final_correction_files':['docs/task-handles.md','benchmarks/results/task-release-0.1.11/provenance.json'], 'release_plan':release_plan, 'changelog_items':14, 'changelog_date':'2026-10-07', 'standalone_runtime_version':standalone['package']['version'], 'archive_names':archives, 'matching_checksums':[v+'.sha256' for v in archives], 'migration_rust_samples_ja_en_equal_and_unchanged':True, 'dynamic_tests_rerun':False, 'source_writes':False}
(OUTPUT/'release-integration-validation.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n')
print(json.dumps(report,ensure_ascii=False,indent=2))
