"""Remove only reviewed CI-level wiring before the frozen historical inverse.

These anchors contain policy wiring; unchanged native command bodies are never
copied or rehashed. Missing, duplicated, or drifted controls fail closed.
"""
import re
import importlib.util
from pathlib import Path


def _sibling(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

EDITS = [(None, "  workflow_dispatch:\n    inputs:\n      level:\n        description: Coverage (release retains all four compiler platforms)\n        type: choice\n        options: [default, release]\n        default: default\n  schedule:\n    - cron: '23 3 * * 1'\n", '  workflow_dispatch:\n', 1), (None, "  ci-plan:\n    needs: [changes, release-plan]\n    if: always() && needs.changes.result == 'success' && needs.release-plan.result == 'success'\n    runs-on: ubuntu-latest\n    outputs:\n      level: ${{ steps.level.outputs.level }}\n      critical_checks: ${{ steps.level.outputs.critical_checks }}\n      comprehensive_checks: ${{ steps.level.outputs.comprehensive_checks }}\n      package_checks: ${{ steps.level.outputs.package_checks }}\n      matrix: ${{ steps.level.outputs.matrix }}\n    steps:\n      - uses: actions/checkout@v4\n        with:\n          fetch-depth: 0\n      - uses: actions/setup-python@v5\n        with:\n          python-version: '3.12'\n      - id: level\n        env:\n          CI_EVENT: ${{ github.event_name }}\n          CI_BASE: ${{ github.event.pull_request.base.sha || github.event.before }}\n          CI_HEAD: ${{ github.sha }}\n          CI_FULL: ${{ needs.changes.outputs.full_checks }}\n          CI_RELEASE_PLAN: ${{ toJSON(needs.release-plan.outputs) }}\n          CI_REQUESTED: ${{ inputs.level || 'default' }}\n        run: python -B scripts/ci/levels.py\n\n  critical:\n    name: Critical compiler and runtime (Linux)\n    needs: [changes, ci-plan]\n    if: needs.changes.result == 'success' && needs.ci-plan.result == 'success' && needs.ci-plan.outputs.critical_checks == 'true'\n    runs-on: ubuntu-latest\n    env:\n      NAGI_FAILURE_DIR: ${{ github.workspace }}/build/compiler-failures\n      NAGI_GOLDEN_CAPTURE_DIR: ${{ github.workspace }}/build/compiler-failures/goldens\n    steps:\n      - uses: actions/checkout@v4\n      - uses: dtolnay/rust-toolchain@stable\n        id: rust\n        with:\n          components: rustfmt, clippy\n      - uses: actions/cache@v4\n        with:\n          path: |\n            ~/.cargo/registry\n            ~/.cargo/git\n            target\n          key: nagi-critical-v1-${{ runner.os }}-${{ runner.arch }}-${{ steps.rust.outputs.cachekey }}-${{ hashFiles('Cargo.lock') }}-${{ hashFiles('Cargo.toml', 'compiler/**', 'runtime/**') }}\n          restore-keys: nagi-critical-v1-${{ runner.os }}-${{ runner.arch }}-${{ steps.rust.outputs.cachekey }}-${{ hashFiles('Cargo.lock') }}-\n      - uses: actions/setup-python@v5\n        with:\n          python-version: '3.12'\n      - run: python -B scripts/ci/critical_inventory.py\n      - run: cargo fmt --all -- --check\n      - run: cargo clippy --locked --all-targets -- -D warnings\n      - run: cargo test --locked -p nagic --lib\n      - run: cargo test --locked -p nagi-runtime --lib\n      - run: cargo test --locked -p nagic --test enum_parser --test builtin_type_contracts\n      - run: cargo test --locked --doc\n      - uses: actions/upload-artifact@v4\n        if: failure()\n        with:\n          name: critical-linux-failures\n          path: |\n            build/compiler-failures/\n            build/ci-evidence/critical-inventory/\n          if-no-files-found: ignore\n\n  jetbrains:\n", '  jetbrains:\n', 1), ('linux', "    needs: [changes, ci-plan]\n    if: needs.changes.result == 'success' && needs.ci-plan.result == 'success' && needs.ci-plan.outputs.comprehensive_checks == 'true'\n", "    needs: changes\n    if: needs.changes.outputs.full_checks == 'true'\n", 1), ('rust-1-88', "    needs: [changes, ci-plan]\n    if: needs.changes.result == 'success' && needs.ci-plan.result == 'success' && needs.ci-plan.outputs.comprehensive_checks == 'true'\n", "    needs: changes\n    if: needs.changes.outputs.full_checks == 'true'\n", 1), ('nagi-package', '    needs: [changes, release-plan, ci-plan]\n', '    needs: [changes, release-plan]\n', 1), ('nagi-package', "      needs.ci-plan.result == 'success' && needs.ci-plan.outputs.package_checks == 'true'\n", "      needs.release-plan.outputs.package_nagi == 'true'\n", 1), ('nagi-package', '      matrix: ${{ fromJSON(needs.ci-plan.outputs.matrix) }}\n', '      matrix:\n        include:\n          - os: ubuntu-latest\n            platform: linux-x86_64\n            executable: nagic\n            rust_host: x86_64-unknown-linux-gnu\n          - os: windows-latest\n            platform: windows-x86_64\n            executable: nagic.exe\n            rust_host: x86_64-pc-windows-msvc\n          - os: macos-15\n            platform: macos-arm64\n            executable: nagic\n            rust_host: aarch64-apple-darwin\n          - os: macos-15-intel\n            platform: macos-x86_64\n            executable: nagic\n            rust_host: x86_64-apple-darwin\n', 1), ('nagi-regression', '    needs: [changes, release-plan, ci-plan]\n', '    needs: [changes, release-plan]\n', 1), ('nagi-regression', "      needs.ci-plan.result == 'success' && needs.ci-plan.outputs.package_checks == 'true'\n", "      needs.release-plan.outputs.package_nagi == 'true'\n", 1), ('nagi-regression', '      matrix: ${{ fromJSON(needs.ci-plan.outputs.matrix) }}\n', '      matrix:\n        include:\n          - os: ubuntu-latest\n            platform: linux-x86_64\n            executable: nagic\n            rust_host: x86_64-unknown-linux-gnu\n          - os: windows-latest\n            platform: windows-x86_64\n            executable: nagic.exe\n            rust_host: x86_64-pc-windows-msvc\n          - os: macos-15\n            platform: macos-arm64\n            executable: nagic\n            rust_host: aarch64-apple-darwin\n          - os: macos-15-intel\n            platform: macos-x86_64\n            executable: nagic\n            rust_host: x86_64-apple-darwin\n', 1), ('sf08-browser', '    needs: [changes, release-plan, ci-plan]\n', '    needs: [changes, release-plan]\n', 1), ('sf08-browser', "      (needs.ci-plan.result == 'success' && (needs.ci-plan.outputs.comprehensive_checks == 'true' || needs.changes.outputs.full_checks == 'true') || needs.release-plan.outputs.package_nagi == 'true')\n", "      (needs.changes.outputs.full_checks == 'true' || needs.release-plan.outputs.package_nagi == 'true')\n", 1), ('ready', '    if: always()\n', "    if: github.event_name == 'pull_request' && always()\n", 1), (None, '    needs: [changes, ci-plan, critical, linux, rust-1-88, jetbrains, release-plan, sf08-browser, vscode-package, nagi-package, nagi-regression]\n', '    needs: [changes, linux, rust-1-88, jetbrains, release-plan, sf08-browser, vscode-package, nagi-package, nagi-regression]\n', 1)]

def inverse_workflow(source):
    source = _sibling('parallel_cache_projection').inverse_workflow(source)
    source = _sibling('windows_contract_projection').inverse_workflow(source)
    source = _sibling('linux_workspace_projection').inverse_workflow(source)
    if '# ci-pin:' in source or '# ci-publication' in source:
        infrastructure = _sibling('optimization_projection')
        publication = _sibling('publication_projection')
        source = publication.inverse_publication(infrastructure.inverse_infrastructure(source))
    for job, added, inherited, count in reversed(EDITS):
        if job is None:
            block = source
        else:
            found = re.search(r'^  ' + re.escape(job) + r':\n.*?(?=^  [a-z][a-z0-9-]*:|\Z)', source, re.M | re.S)
            if found is None:
                raise ValueError('CI-level job missing')
            block = found.group()
        if block.count(added) != count:
            raise ValueError('CI-level policy inverse anchor mismatch: ' + str(job))
        restored = block.replace(added, inherited)
        if job is None:
            source = restored
        else:
            source = source[:found.start()] + restored + source[found.end():]
    return source
