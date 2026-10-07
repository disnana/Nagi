from pathlib import Path
import argparse, hashlib, json, subprocess, tarfile, tomllib, zipfile

p = argparse.ArgumentParser()
p.add_argument('--metadata', type=Path, required=True)
p.add_argument('--commit', required=True)
p.add_argument('--out', type=Path, required=True)
a = p.parse_args()
r = json.loads(a.metadata.read_text())
version = '0.1.11'
tag = 'nagi-v' + version
assert r['tag_name'] == tag and not r['draft'] and not r['prerelease'] and r['published_at']
assets = {x['name']: x for x in r['assets']}
platforms = ('linux-x86_64', 'macos-arm64', 'macos-x86_64', 'windows-x86_64')
archives = {x: f'nagi-{version}-{x}' + ('.zip' if x.startswith('windows') else '.tar.gz') for x in platforms}
expected = {n for name in archives.values() for n in (name, name + '.sha256')}
assert len(r['assets']) == len(expected) and set(assets) == expected, sorted(assets)
assert 'nagi-v0.1.10' in r['body'] and '815b7d554dba3ac6b11ddad2089b782cbed46eb1' in r['body']
assert 'Task' in r['body'] and '0.1.11' in r['body']
a.out.mkdir(parents=True, exist_ok=True)
report = {'tag': tag, 'commit': a.commit, 'published_at': r['published_at'], 'release_url': r['html_url'], 'assets': [], 'archives': []}
for name in sorted(expected):
    dest = a.out / name
    url = f'https://github.com/disnana/Nagi/releases/download/{tag}/{name}'
    assert assets[name]['browser_download_url'] == url
    subprocess.run(['curl', '--fail', '--location', '--retry', '2', '--connect-timeout', '20', '--max-time', '180', '--silent', '--show-error', '--output', str(dest), url], check=True)
    digest = hashlib.sha256(dest.read_bytes()).hexdigest()
    assert dest.stat().st_size == assets[name]['size'], name
    remote_digest = assets[name].get('digest')
    if remote_digest is not None:
        assert remote_digest == 'sha256:' + digest, (name, remote_digest, digest)
    report['assets'].append({'name': name, 'bytes': dest.stat().st_size, 'sha256': digest, 'github_digest': remote_digest})
    print('Downloaded and checked:', name, flush=True)
for platform, name in archives.items():
    dest = a.out / name
    digest = hashlib.sha256(dest.read_bytes()).hexdigest()
    assert (a.out / (name + '.sha256')).read_text().strip() == f'{digest}  {name}', name
    stem = f'nagi-{version}-{platform}'
    if dest.suffix == '.zip':
        with zipfile.ZipFile(dest) as z:
            metadata = json.loads(z.read(stem + '/release.json'))
            manifest = tomllib.loads(z.read(stem + '/runtime/Cargo.toml').decode())
            members = z.namelist()
    else:
        with tarfile.open(dest) as t:
            metadata = json.loads(t.extractfile(stem + '/release.json').read())
            manifest = tomllib.loads(t.extractfile(stem + '/runtime/Cargo.toml').read().decode())
            members = t.getnames()
    assert metadata == {'version': version, 'platform': platform, 'commit': a.commit}, metadata
    assert manifest['package']['version'] == version and isinstance(manifest['package']['version'], str)
    assert all('/.git/' not in x and '/target/' not in x and '/compiler/' not in x and '/benchmarks/' not in x for x in members)
    report['archives'].append({'name': name, 'checksum_sidecar_matched': True, 'release_json': metadata, 'standalone_runtime_version': manifest['package']['version']})
    print('Verified archive identity:', platform, flush=True)
(a.out / 'published-assets-verified.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n')
print('All eight published assets and four archive identities verified.', flush=True)
