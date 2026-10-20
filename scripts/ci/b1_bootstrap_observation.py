"""Temporary B1 failure observation; import constants/helpers into owned projection.

This isolated preparation does not modify a workflow or its historical digest.
The supplementary cfg run observes a new condition and never replaces acceptance.
"""
import re

# Record provenance: arm 0 is unbound, 1 is cold, 2 is ready.

B1_NAME = "      - name: Check B1 Session CSRF runtime and public socket contracts\n"
B1_IDENTIFIED = B1_NAME + "        id: b1_csrf_runtime\n"
B1_NEXT = "      - name: Check B1 CSRF compiler metadata and High saved Low handwritten Low native\n"
STEP = r'''      - name: Observe B1 bootstrap only after original acceptance failure
        if: failure() && steps.b1_csrf_runtime.outcome == 'failure'
        shell: bash
        run: |
          set -euo pipefail
          python - <<'PYB1RESOURCE'
          import math, os, pathlib, shutil
          cpu = os.cpu_count() or 0
          cpu = cpu if 0 < cpu <= 65535 else 0
          load_ok, load = 0, [0, 0, 0]
          try:
              values = os.getloadavg()
              if len(values) == 3 and all(math.isfinite(v) and 0 <= v <= 1000000 for v in values):
                  load_ok, load = 1, [int(v * 1000) for v in values]
          except (AttributeError, OSError):
              pass
          memory_ok, memory = 0, [0, 0, 0, 0, 0]
          try:
              with open('/proc/meminfo', 'rb') as source:
                  raw = source.read(4097)
              fields = {}
              if len(raw) <= 4096:
                  for line in raw.decode('ascii').splitlines():
                      key, _, rest = line.partition(':')
                      if key in ('MemTotal', 'MemAvailable', 'SwapFree', 'Dirty', 'Writeback'):
                          value, unit = rest.split()
                          if unit != 'kB' or not value.isdecimal() or len(value) > 16:
                              raise ValueError('invalid memory field')
                          fields[key] = int(value)
                  if len(fields) == 5:
                      memory_ok = 1
                      memory = [fields[key] for key in ('MemTotal', 'MemAvailable', 'SwapFree', 'Dirty', 'Writeback')]
          except (OSError, UnicodeError, ValueError):
              pass
          disk_ok, disk = 0, [0, 0]
          try:
              usage = shutil.disk_usage(pathlib.Path(os.environ['RUNNER_TEMP']))
              if 0 <= usage.free <= usage.total < 2**64:
                  disk_ok, disk = 1, [usage.total, usage.free]
          except (KeyError, OSError):
              pass
          print('b1_supplement_resources cpu={} load_supported={} load1_milli={} load5_milli={} load15_milli={} memory_supported={} mem_total_kib={} mem_available_kib={} swap_free_kib={} dirty_kib={} writeback_kib={} disk_supported={} disk_total_bytes={} disk_free_bytes={}'.format(cpu, load_ok, *load, memory_ok, *memory, disk_ok, *disk))
          # Unsupported resource fields remain explicitly unavailable.
          PYB1RESOURCE
          set +e
          CARGO_TERM_COLOR=never RUSTFLAGS='--cfg nagi_auth_stage_diag' cargo test --locked -p nagi-runtime --lib http_server::csrf_http::deadline_tests::pool_readiness_tests:: -- --show-output 2>&1 | python -c 'import sys
          remaining=256*1024+1
          while True:
           chunk=sys.stdin.buffer.read(8192)
           if not chunk:
            break
           if remaining:
            sys.stdout.buffer.write(chunk[:remaining])
            remaining-=min(remaining,len(chunk))
          sys.stdout.buffer.flush()' > "$RUNNER_TEMP/b1-bootstrap-supplement.raw"
          b1_diag_status=("${PIPESTATUS[@]}")
          set -e
          B1_DIAGNOSTIC_CARGO_RC="${b1_diag_status[0]}" B1_DIAGNOSTIC_CAPTURE_RC="${b1_diag_status[1]}" python - <<'PYB1LOG'
          import os, pathlib, re
          path = pathlib.Path(os.environ['RUNNER_TEMP']) / 'b1-bootstrap-supplement.raw'
          raw, read_ok = b'', 0
          try:
              with path.open('rb') as source:
                  raw = source.read(256 * 1024 + 1)
              read_ok = 1
          except OSError:
              pass
          finally:
              try:
                  path.unlink(missing_ok=True)
              except OSError:
                  read_ok = 0
          def code(name):
              value = os.environ.get(name, '')
              return int(value) if re.fullmatch(r'[0-9]{1,3}', value) and int(value) <= 255 else 255
          cargo_rc = code('B1_DIAGNOSTIC_CARGO_RC')
          capture_rc = code('B1_DIAGNOSTIC_CAPTURE_RC')
          fields = {
              'session_deadline_diag': 'status phase event db exit polls begins commands commits budget_ms entry_remaining_ms phase_remaining_ms exit_remaining_ms',
              'session_private_deadline_diag': 'status phase event db polls begins commands commits budget_ms entry_remaining_ms phase_remaining_ms exit_remaining_ms terminal',
              'csrf_token_deadline_diag': 'status phase event db exit polls begins commands commits budget_ms entry_remaining_ms phase_remaining_ms exit_remaining_ms terminal',
              'csrf_guarded_deadline_diag': 'status phase event db exit polls begins commands commits budget_ms entry_remaining_ms phase_remaining_ms exit_remaining_ms selected_deadline_passed terminal',
              'session_native_finish_diag': 'status origin terminal diagnostic_valid native_state commit_result cleanup_result cleanup_reason commit_enter_us commit_return_us cleanup_enter_us cleanup_return_us settled_us begins commits selected_deadline_passed',
              'session_native_call_diag': 'status origin terminal diagnostic_valid native_call_state generation call_enter_us call_return_us native_state commit_result settled_us begins commits selected_deadline_passed',
              'b1_startup_bootstrap': 'stage status created idle elapsed_us',
              'b1_startup_bootstrap_gate': 'stage created idle elapsed_us',
          }
          patterns = {prefix: re.compile(prefix + ''.join(' ' + key + '=' + (r'(?:issue|csrf)' if key == 'stage' else r'[0-9]{1,20}') for key in keys.split())) for prefix, keys in fields.items()}
          case_root = 'http_server::csrf_http::deadline_tests::pool_readiness_tests::'
          cases = ('cold_worker2_startup_spends_original_guarded_http_budget', 'ready_worker2_setup_preserves_same_guarded_http_budget')
          inventory = re.compile(r'^test ' + re.escape(case_root) + '(' + '|'.join(cases) + r') \.\.\. (ok|FAILED)$')
          summary = re.compile(r'^test result: (ok|FAILED)\. ([0-9]+) passed; ([0-9]+) failed; ([0-9]+) ignored; ([0-9]+) measured; ([0-9]+) filtered out; finished in [0-9]+(?:\.[0-9]+)?s$')
          heading = re.compile(r'^---- ' + re.escape(case_root) + '(' + '|'.join(re.escape(name) for name in cases) + r') stdout ----$')
          arm = 0
          rows, seen, summaries, malformed = [], [], [], 0
          for line in raw.decode('utf-8', errors='replace').splitlines():
              if line.startswith('---- ') and line.endswith(' stdout ----'):
                  match = heading.fullmatch(line)
                  arm = cases.index(match.group(1)) + 1 if match else 0
              elif line.startswith(('test result:', 'successes:', 'failures:')):
                  arm = 0
              prefix = line.split(' ', 1)[0]
              if prefix in patterns:
                  if len(line) <= 1024 and patterns[prefix].fullmatch(line) and len(rows) < 64:
                      rows.append((arm, line))
                  else:
                      malformed = 1
              found = inventory.fullmatch(line)
              if found:
                  seen.append(found.groups())
              found = summary.fullmatch(line)
              if found:
                  summaries.append(found.groups())
          overflow = int(len(raw) > 256 * 1024)
          inventory_ok = int(len(seen) == 2 and sorted(name for name, _ in seen) == sorted(cases))
          summary_ok = 0
          if inventory_ok and len(summaries) == 1:
              kind, passed, failed, ignored, measured, _ = summaries[0]
              passed, failed = int(passed), int(failed)
              expected_passed = sum(state == 'ok' for _, state in seen)
              summary_ok = int(passed == expected_passed and passed + failed == 2 and ignored == '0' and measured == '0' and ((cargo_rc == 0 and kind == 'ok' and failed == 0) or (cargo_rc != 0 and kind == 'FAILED' and failed > 0)))
          bootstrap = [row for _, row in rows if row.startswith('b1_startup_bootstrap')]
          bootstrap_ok = int(bool(bootstrap))
          if cargo_rc == 0:
              bootstrap_ok = int(len(bootstrap) == 4 and sum(row.startswith('b1_startup_bootstrap stage=issue status=200 ') for row in bootstrap) == 2 and sum(row.startswith('b1_startup_bootstrap stage=csrf status=200 ') for row in bootstrap) == 2)
          available = int(read_ok and not capture_rc and not overflow and not malformed and inventory_ok and summary_ok and bootstrap_ok)
          if not overflow and not malformed:
              for arm, row in rows:
                  print('b1_supplement_record arm={} {}'.format(arm, row))
          print('b1_supplement_result cargo_rc={} capture_rc={} captured_bytes={} overflow={} malformed={} records={} inventory_ok={} summary_ok={} bootstrap_ok={} diagnostic_available={}'.format(cargo_rc, capture_rc, len(raw), overflow, malformed, len(rows), inventory_ok, summary_ok, bootstrap_ok, available))
          raise SystemExit(cargo_rc if available else 1)
          PYB1LOG
'''


_RESOURCE_START = STEP.index("          python - <<'PYB1RESOURCE'\n")
_RESOURCE_END = STEP.index("          set +e\n", _RESOURCE_START)
RESOURCE_STEP = (
    "      - name: Observe resources before the B1 acceptance group\n"
    "        shell: bash\n"
    "        run: |\n"
    + STEP[_RESOURCE_START:_RESOURCE_END].replace(
        "b1_supplement_resources", "b1_bootstrap_resources")
)


def add_failure_diagnostic(source):
    """Insert twice immediately after B1; leave every original body byte intact."""
    if source.count(B1_NAME) != 2 or B1_IDENTIFIED in source or STEP in source:
        raise ValueError('B1 supplementary source inventory mismatch')
    pattern = re.compile(re.escape(B1_NAME) + r'(        shell: bash\n(?:(?!^      - name:).)*?)' + r'(?=^      - name:)', re.MULTILINE | re.DOTALL)
    converted, count = pattern.subn(lambda match: RESOURCE_STEP + B1_IDENTIFIED + match.group(1) + STEP, source)
    if count != 2 or inverse_failure_diagnostic(converted) != source:
        raise ValueError('B1 supplementary insertion mismatch')
    return converted


def inverse_failure_diagnostic(source):
    """Call before inherited shared inverse; reject missing/moved/changed additions."""
    if source.count(B1_IDENTIFIED) != 2 or source.count(STEP) != 2 or source.count(RESOURCE_STEP) != 2:
        raise ValueError('B1 supplementary inverse inventory mismatch')
    pattern = re.compile(re.escape(RESOURCE_STEP + B1_IDENTIFIED) + r'(        shell: bash\n(?:(?!^      - name:).)*?)' + re.escape(STEP) + r'(?=^      - name:)', re.MULTILINE | re.DOTALL)
    inherited, count = pattern.subn(lambda match: B1_NAME + match.group(1), source)
    if count != 2 or 'id: b1_csrf_runtime' in inherited or 'b1_supplement_' in inherited:
        raise ValueError('B1 supplementary inverse placement mismatch')
    return inherited
