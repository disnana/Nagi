"""Compile and require nonempty named inventories for every fixed L1 test target."""
import json
import re
import subprocess
from pathlib import Path

GROUPS = [('nagic', ['--lib'], {'nagic'}), ('nagi-runtime', ['--lib'], {'nagi_runtime'}),
          ('nagic', ['--test', 'enum_parser', '--test', 'builtin_type_contracts'], {'enum_parser', 'builtin_type_contracts'})]


def binaries(text, expected):
    found = {}
    for line in text.splitlines():
        message = json.loads(line)
        if message.get('reason') != 'compiler-artifact' or not message.get('profile', {}).get('test') or not message.get('executable'):
            continue
        name = message['target']['name']
        if name not in expected or name in found:
            raise ValueError('unexpected or duplicate compiled critical target')
        found[name] = message['executable']
    if set(found) != expected:
        raise ValueError('missing compiled critical target')
    return found


def cases(text):
    names = []
    for line in text.splitlines():
        match = re.fullmatch(r'([^\r\n]+): test', line)
        if match:
            names.append(match[1])
        elif line and not re.fullmatch(r'\d+ tests?, \d+ benchmarks?', line):
            raise ValueError('unexpected libtest inventory line')
    if not names or len(names) != len(set(names)):
        raise ValueError('empty or duplicate critical test inventory')
    return sorted(names)


def main():
    evidence = Path('build/ci-evidence/critical-inventory')
    evidence.mkdir(parents=True, exist_ok=True)
    result = {}
    for package, targets, expected in GROUPS:
        command = ['cargo', 'test', '--locked', '-p', package, *targets, '--no-run', '--message-format=json']
        build = subprocess.run(command, text=True, stdout=subprocess.PIPE)
        (evidence / (package + '-' + '-'.join(sorted(expected)) + '.jsonl')).write_text(build.stdout, encoding='utf-8')
        build.check_returncode()
        for name, executable in binaries(build.stdout, expected).items():
            listed = subprocess.run([executable, '--list', '--format=terse'], text=True, stdout=subprocess.PIPE, check=True)
            result[name] = cases(listed.stdout)
    (evidence / 'inventory.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print('Critical targets:', ', '.join(f'{name}={len(names)}' for name, names in sorted(result.items())))

if __name__ == '__main__':main()
