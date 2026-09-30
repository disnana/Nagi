"""source、生成Low/Rust、docs、生ログをzipへまとめてCRC/sha256を確認する。"""
import argparse, hashlib, json, re, tempfile, zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXCLUDED = {'target', 'native-target', '.git', '__pycache__'}

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--out', type=Path, default=ROOT.parent / 'Nagi-prototype.zip')
    args = parser.parse_args()
    archive = args.out.resolve()
    missing = []
    for path in [ROOT / 'README.md', ROOT / 'PERFORMANCE.md', *sorted((ROOT / 'docs').glob('*.md'))]:
        prose = re.sub(r'```[\s\S]*?```', '', path.read_text())
        prose = re.sub(r'`[^`]*`', '', prose)
        for target in re.findall(r'\[[^\]]*\]\(([^)]+)\)', prose):
            if not target.startswith(('http:', 'https:', '#')) and not (path.parent / target.split('#')[0]).exists():
                missing.append((str(path.relative_to(ROOT)), target))
    assert not missing, missing
    files = sorted(p for p in ROOT.rglob('*') if p.is_file() and p.resolve() != archive and not any(v in EXCLUDED for v in p.relative_to(ROOT).parts) and p.name != 'unit-tests.txt')
    manifest_path = ROOT / 'benchmarks/results/source-sha256.json'
    manifest = {str(p.relative_to(ROOT)): hashlib.sha256(p.read_bytes()).hexdigest() for p in files if p != manifest_path}
    manifest_path.write_text(json.dumps(manifest, indent=2))
    files = sorted(set(files + [manifest_path]))
    archive.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(archive, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as bundle:
        for path in files:
            bundle.write(path, 'nagi/' + str(path.relative_to(ROOT)))
    with zipfile.ZipFile(archive) as bundle:
        assert bundle.testzip() is None
        assert 'nagi/compiler/src/main.rs' in bundle.namelist() and 'nagi/PERFORMANCE.md' in bundle.namelist()
        with tempfile.TemporaryDirectory(prefix='nagi-archive-') as directory:
            extracted = Path(directory)
            bundle.extractall(extracted)
            for name, digest in manifest.items():
                assert hashlib.sha256((extracted / 'nagi' / name).read_bytes()).hexdigest() == digest, name
    print(json.dumps({'archive': str(archive), 'bytes': archive.stat().st_size, 'files': len(files), 'broken_links': 0, 'crc': 'passed', 'extracted_sha256': 'passed'}))

if __name__ == '__main__':
    main()
