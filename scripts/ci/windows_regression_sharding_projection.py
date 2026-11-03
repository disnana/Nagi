"""Exact outer Windows regression shard wiring over verified #221."""
import hashlib
import importlib.util
import re
import sys
from pathlib import Path


BASE_WORKFLOW_SHA256 = "f9548a1c22c71776559b1593bd1a5aafc6b3df8b6fd91b054609e41993b28691"
JOB = re.compile(r"(?ms)^  (?P<name>[a-z][a-z0-9-]*):\n.*?(?=^  [a-z][a-z0-9-]*:\n|\Z)")
STEP = re.compile(r"(?ms)^      - .*?(?=^      - |\Z)")
REGRESSION_GUARD = "matrix.platform != 'windows-x86_64'"
HANDOFF_STEP = """      - name: Delegate Windows regression targets to strict shard aggregate
        if: matrix.platform == 'windows-x86_64'
        shell: pwsh
        run: Write-Output 'The mandatory two-shard Windows regression and aggregate run in the dedicated Windows jobs.'
"""


def _original_commands():
    scripts = Path(__file__).resolve().parent
    spec = importlib.util.spec_from_file_location(
        "windows_regression_sharding", scripts / "windows_regression_sharding.py")
    module = importlib.util.module_from_spec(spec)
    sys.path.insert(0, str(scripts))
    try:
        spec.loader.exec_module(module)
    finally:
        sys.path.remove(str(scripts))
    return module.ORIGINAL_COMMANDS

SHARDS_JOB = """  nagi-regression-windows-shards:
    name: Windows compiler regression shard ${{ matrix.shard }}
    needs: [changes, ci-plan]
    if: needs.changes.result == 'success' && needs.ci-plan.result == 'success' && needs.ci-plan.outputs.package_checks == 'true'
    runs-on: windows-latest
    strategy:
      fail-fast: false
      matrix:
        shard: [0, 1]
    env:
      NAGI_FAILURE_DIR: ${{ github.workspace }}/build/compiler-failures
      NAGI_GOLDEN_CAPTURE_DIR: ${{ github.workspace }}/build/compiler-failures/goldens
    steps:
      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # ci-pin:v4
        with:
          persist-credentials: false # ci-isolation
          fetch-depth: 0
          ref: ${{ github.event.pull_request.head.sha || github.sha }}
      - uses: dtolnay/rust-toolchain@b725eda525362e7cf34af9a7243cac1e3f01a7ec # ci-pin:stable
        id: rust
      - uses: actions/setup-python@a26af69be951a213d495a4c3e4e4022e16d87065 # ci-pin:v5
        with:
          python-version: '3.12'
      - uses: actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020 # ci-pin:v4
        with:
          node-version: '22'
      - name: Restore only this Windows regression shard's default-profile cache
        uses: actions/cache/restore@0057852bfaa89a56745cba8c7296529d2fc39830 # ci-pin:v4
        id: nagi_windows_regression_cache
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
            native-target
          key: nagi-rust-v2-windows-regression-${{ matrix.shard }}-${{ runner.os }}-${{ runner.arch }}-${{ steps.rust.outputs.cachekey }}-profile-default-incr-default-rustflags-default-${{ hashFiles('Cargo.lock') }}-${{ hashFiles('.cargo/config', '.cargo/config.toml', 'compiler/.cargo/config', 'compiler/.cargo/config.toml') }}-${{ hashFiles('.github/workflows/ci.yml', 'scripts/ci/windows_regression_sharding.py', 'scripts/ci/windows_regression_sharding_projection.py', 'scripts/ci/linux_critical_path.py', 'scripts/ci/nextest_evaluation.py') }}-${{ hashFiles('Cargo.toml', 'compiler/**', 'runtime/**', 'runtime-core/**') }}
      - name: Execute the complete reviewed Windows target shard
        env:
          NAGI_REGRESSION_EXPECTED_HEAD: ${{ github.event.pull_request.head.sha || github.sha }}
          SHARD: ${{ matrix.shard }}
        shell: pwsh
        run: python -B scripts/ci/windows_regression_sharding.py --shard "$env:SHARD"
      - name: Preserve this Windows regression shard receipt and logs
        if: always()
        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # ci-pin:v4
        with:
          retention-days: 7 # ci-retention
          name: nagi-regression-windows-shard-${{ matrix.shard }}-attempt-${{ github.run_attempt }}
          path: build/ci-evidence/windows-regression/shard-${{ matrix.shard }}/
          if-no-files-found: error
      - name: Preserve Windows regression compiler and application failure evidence
        if: failure()
        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # ci-pin:v4
        with:
          retention-days: 7 # ci-retention
          name: compiler-failures-windows-regression-shard-${{ matrix.shard }}-attempt-${{ github.run_attempt }}
          path: |
            build/compiler-failures/
            build/ci-evidence/windows-regression/shard-${{ matrix.shard }}/
          if-no-files-found: error
      - name: Save validated Windows regression shard cache (main only) # ci-cache-save
        if: success() && github.event_name == 'push' && github.ref == 'refs/heads/main' && steps.nagi_windows_regression_cache.outputs.cache-hit != 'true' && steps.nagi_windows_regression_cache.outputs.cache-primary-key != ''
        uses: actions/cache/save@0057852bfaa89a56745cba8c7296529d2fc39830 # ci-pin:v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
            native-target
          key: ${{ steps.nagi_windows_regression_cache.outputs.cache-primary-key }}
"""

AGGREGATE_JOB = """  nagi-regression-windows-aggregate:
    name: Reconcile Windows compiler regression shards
    needs: [changes, ci-plan, nagi-regression-windows-shards]
    if: >-
      always() && needs.changes.result == 'success' && needs.ci-plan.result == 'success' &&
      needs.ci-plan.outputs.package_checks == 'true'
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@11d5960a326750d5838078e36cf38b85af677262 # ci-pin:v4
        with:
          persist-credentials: false # ci-isolation
          fetch-depth: 0
          ref: ${{ github.event.pull_request.head.sha || github.sha }}
      - uses: dtolnay/rust-toolchain@b725eda525362e7cf34af9a7243cac1e3f01a7ec # ci-pin:stable
      - uses: actions/setup-python@a26af69be951a213d495a4c3e4e4022e16d87065 # ci-pin:v5
        with:
          python-version: '3.12'
      - name: Download shard 0 from this exact run attempt
        if: always()
        continue-on-error: true
        uses: actions/download-artifact@d3f86a106a0bac45b974a628896c90dbdf5c8093 # ci-pin:v4
        with:
          name: nagi-regression-windows-shard-0-attempt-${{ github.run_attempt }}
          path: build/ci-evidence/windows-regression/artifacts/nagi-regression-windows-shard-0
      - name: Download shard 1 from this exact run attempt
        if: always()
        continue-on-error: true
        uses: actions/download-artifact@d3f86a106a0bac45b974a628896c90dbdf5c8093 # ci-pin:v4
        with:
          name: nagi-regression-windows-shard-1-attempt-${{ github.run_attempt }}
          path: build/ci-evidence/windows-regression/artifacts/nagi-regression-windows-shard-1
      - name: Reconcile the two complete Windows regression executions
        if: always()
        env:
          NAGI_REGRESSION_EXPECTED_HEAD: ${{ github.event.pull_request.head.sha || github.sha }}
          NAGI_REGRESSION_SHARDS_RESULT: ${{ needs.nagi-regression-windows-shards.result }}
        shell: pwsh
        run: python -B scripts/ci/windows_regression_sharding.py --aggregate --receipts build/ci-evidence/windows-regression/artifacts
      - name: Preserve Windows regression aggregate diagnostics
        if: always()
        uses: actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # ci-pin:v4
        with:
          retention-days: 7 # ci-retention
          name: nagi-regression-windows-aggregate-attempt-${{ github.run_attempt }}
          path: build/ci-evidence/windows-regression/aggregate/
          if-no-files-found: ignore
"""

OLD_READY_NEEDS = "    needs: [changes, ci-plan, critical, go-backend, linux, linux-workspace, linux-csrf-mutants, linux-samples, rust-1-88, jetbrains, release-plan, sf08-browser, vscode-package, nagi-package, nagi-regression, platform-contracts]\n"
NEW_READY_NEEDS = OLD_READY_NEEDS.replace(
    "nagi-regression, platform-contracts",
    "nagi-regression, nagi-regression-windows-shards, nagi-regression-windows-aggregate, platform-contracts")
OLD_PUBLISH_NEEDS = "    needs: [changes, ci-plan, critical, go-backend, linux, linux-workspace, linux-csrf-mutants, linux-samples, rust-1-88, sf08-browser, release-plan, jetbrains, vscode-package, nagi-package, nagi-regression, platform-contracts, ready]\n"
NEW_PUBLISH_NEEDS = OLD_PUBLISH_NEEDS.replace(
    "nagi-regression, platform-contracts",
    "nagi-regression, nagi-regression-windows-shards, nagi-regression-windows-aggregate, platform-contracts")


def _workflow_hash(source):
    return hashlib.sha256(source.encode("utf-8")).hexdigest()


def _job(source, name):
    found = [match for match in JOB.finditer(source) if match.group("name") == name]
    if len(found) != 1:
        raise ValueError("missing or duplicated workflow job: " + name)
    return found[0]


def _rewrite_regression(block, *, inverse=False):
    found = list(STEP.finditer(block))
    if len(found) != (12 if inverse else 11):
        raise ValueError("Windows matrix regression step inventory changed")
    before, steps, after = block[:found[0].start()], [match.group() for match in found], block[found[-1].end():]
    if inverse:
        if steps[-1] != HANDOFF_STEP:
            raise ValueError("Windows regression handoff diagnostic changed or moved")
        steps.pop()
        if len(steps) != 11:
            raise ValueError("Windows matrix regression step count changed")
        for index, step in enumerate(steps):
            if index == 9:
                current = "if: failure() && " + REGRESSION_GUARD
                original = "if: failure()"
                if step.count(current + "\n") != 1:
                    raise ValueError("Windows matrix failure-artifact guard changed")
                step = step.replace(current + "\n", original + "\n", 1)
            elif index == 10:
                original = "if: success() && github.event_name == 'push' && github.ref == 'refs/heads/main' && steps.nagi_rust_cache.outputs.cache-hit != 'true' && steps.nagi_rust_cache.outputs.cache-primary-key != ''"
                current = original + " && " + REGRESSION_GUARD
                if step.count(current + "\n") != 1:
                    raise ValueError("Windows matrix cache-save guard changed")
                step = step.replace(current + "\n", original + "\n", 1)
            else:
                guard_line = "        if: " + REGRESSION_GUARD + "\n"
                if step.count(guard_line) != 1:
                    raise ValueError("Windows matrix non-test step guard changed")
                step = step.replace(guard_line, "", 1)
            steps[index] = step
    else:
        if len(steps) != 11:
            raise ValueError("Windows matrix regression step inventory changed")
        for index, step in enumerate(steps):
            if index == 9:
                if step.count("        if: failure()\n") != 1:
                    raise ValueError("Windows matrix failure-artifact step changed")
                step = step.replace("        if: failure()\n",
                                    "        if: failure() && " + REGRESSION_GUARD + "\n", 1)
            elif index == 10:
                original = "if: success() && github.event_name == 'push' && github.ref == 'refs/heads/main' && steps.nagi_rust_cache.outputs.cache-hit != 'true' && steps.nagi_rust_cache.outputs.cache-primary-key != ''"
                if step.count(original + "\n") != 1:
                    raise ValueError("Windows matrix cache-save step changed")
                step = step.replace(original + "\n", original + " && " + REGRESSION_GUARD + "\n", 1)
            else:
                first_line_end = step.find("\n") + 1
                if first_line_end <= 0 or "        if:" in step:
                    raise ValueError("Windows matrix step already has an unreviewed condition")
                step = step[:first_line_end] + "        if: " + REGRESSION_GUARD + "\n" + step[first_line_end:]
            steps[index] = step
        steps.append(HANDOFF_STEP)
    return before + "".join(steps) + after


def _apply_layer(source):
    if "  nagi-regression-windows-shards:\n" in source or "  nagi-regression-windows-aggregate:\n" in source:
        raise ValueError("Windows regression shard jobs already exist")
    regression = _job(source, "nagi-regression")
    original = regression.group()
    commands = re.findall(r"(?m)^        run: (cargo test --locked -p nagic .*)$", original)
    if tuple(commands) != _original_commands():
        raise ValueError("frozen Windows regression Cargo commands changed")
    rewritten = _rewrite_regression(original)
    source = source[:regression.start()] + rewritten + source[regression.end():]
    ready = _job(source, "ready")
    if ready.group().count(OLD_READY_NEEDS) != 1 or ready.group().count("    if: always() && !cancelled()\n") != 1:
        raise ValueError("Ready gate anchors changed")
    new_ready = ready.group().replace(OLD_READY_NEEDS, NEW_READY_NEEDS, 1)
    source = source[:ready.start()] + new_ready + source[ready.end():]
    publish = _job(source, "publish-release")
    if publish.group().count(OLD_PUBLISH_NEEDS) != 1:
        raise ValueError("release publication dependency anchor changed")
    new_publish = publish.group().replace(OLD_PUBLISH_NEEDS, NEW_PUBLISH_NEEDS, 1)
    source = source[:publish.start()] + new_publish + source[publish.end():]
    if source.count("  ready:\n") != 1:
        raise ValueError("Ready insertion anchor changed")
    return source.replace("  ready:\n", SHARDS_JOB + AGGREGATE_JOB + "  ready:\n", 1)


def apply_workflow(source):
    if _workflow_hash(source) != BASE_WORKFLOW_SHA256:
        raise ValueError("Windows regression shard base is not verified #221")
    return _apply_layer(source)


def inverse_workflow(source):
    if ("  nagi-regression-windows-shards:\n" not in source and
            "  nagi-regression-windows-aggregate:\n" not in source and
            REGRESSION_GUARD not in _job(source, "nagi-regression").group() and
            "nagi-regression-windows-aggregate" not in _job(source, "ready").group()):
        return source
    edited_source = source
    shards = _job(source, "nagi-regression-windows-shards")
    aggregate = _job(source, "nagi-regression-windows-aggregate")
    if shards.group() != SHARDS_JOB or aggregate.group() != AGGREGATE_JOB:
        raise ValueError("Windows regression shard or aggregate job changed")
    source = source[:aggregate.start()] + source[aggregate.end():]
    shards = _job(source, "nagi-regression-windows-shards")
    source = source[:shards.start()] + source[shards.end():]
    regression = _job(source, "nagi-regression")
    restored = _rewrite_regression(regression.group(), inverse=True)
    source = source[:regression.start()] + restored + source[regression.end():]
    ready = _job(source, "ready")
    if (ready.group().count(NEW_READY_NEEDS) != 1 or
            ready.group().count("    if: always() && !cancelled()\n") != 1):
        raise ValueError("Ready Windows aggregate dependency or cancellation guard changed")
    old_ready = ready.group().replace(NEW_READY_NEEDS, OLD_READY_NEEDS, 1)
    source = source[:ready.start()] + old_ready + source[ready.end():]
    publish = _job(source, "publish-release")
    if publish.group().count(NEW_PUBLISH_NEEDS) != 1:
        raise ValueError("publication Windows aggregate dependency changed")
    old_publish = publish.group().replace(NEW_PUBLISH_NEEDS, OLD_PUBLISH_NEEDS, 1)
    source = source[:publish.start()] + old_publish + source[publish.end():]
    if _apply_layer(source) != edited_source:
        raise ValueError("Windows regression inverse did not restore its own exact projection")
    return source
