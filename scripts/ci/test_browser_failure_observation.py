"""Control-flow proof using the actual primary shell and inherited sanitizer."""
import hashlib
import itertools
import json
import os
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tempfile
import textwrap
import unittest

import browser_failure_projection as observation
import sf07_request_projection as history
from sf07_projection_contract import BASE_WORKFLOW_SHA256

ROOT = Path(__file__).resolve().parents[2]


def step(source, title, job="linux"):
    block = re.search(r"(?ms)^  " + re.escape(job) + r":\n.*?(?=^  [a-z][a-z0-9-]*:|\Z)", source).group()
    return re.search(r"(?ms)^      - name: " + re.escape(title) + r"\n.*?(?=^      - |\Z)", block).group()


class BrowserFailureObservationTests(unittest.TestCase):
    def setUp(self):
        self.source = (ROOT / ".github/workflows/ci.yml").read_text()
        self.primary = step(self.source, "Compare BrowserLogin failure cases alone and with the parallel public suite")
        self.capture = step(self.source, "Capture bounded Session deadline public tests")

    def test_inverse_changes_only_linux_metadata_and_keeps_frozen_workflow(self):
        inherited = observation.inverse_workflow(self.source)
        self.assertEqual(self.source.count(observation.IDENTIFIED_PRIMARY), 1)
        self.assertEqual(self.source.count(observation.REACHABLE_CAPTURE), 1)
        self.assertEqual(self.primary.replace("        id: browser_counter_comparison\n", ""),
                         history.BROWSER_COUNTER_PROFILE_STEP)
        self.assertEqual(step(inherited, "Capture bounded Session deadline public tests"),
                         self.capture.replace(observation.CONDITION, ""))
        self.assertEqual(step(inherited, "Capture bounded Session deadline public tests", "nagi-package"),
                         step(self.source, "Capture bounded Session deadline public tests", "nagi-package"))
        self.assertEqual(hashlib.sha256(history.inverse_workflow(self.source).encode()).hexdigest(),
                         BASE_WORKFLOW_SHA256)
        self.assertNotIn("continue-on-error", self.primary)
        self.assertNotIn("if:", self.primary)
        self.assertNotIn("timeout", self.primary)
        self.assertEqual(self.source.count("run: test '${{ steps.session_deadline_diag.outcome }}' = success"), 2)

    def test_actual_condition_only_adds_own_failure_and_never_cancel_or_skip(self):
        condition = self.capture.split("        if: >-\n", 1)[1].split("        continue-on-error:", 1)[0]
        condition = " ".join(condition.split())
        expected = ("!cancelled() && ((success() && steps.browser_counter_comparison.outcome == 'success') || "
                    "(failure() && steps.browser_counter_comparison.outcome == 'failure'))")
        self.assertEqual(condition, expected)
        for primary, preceding, cancelled in itertools.product(
                ("success", "failure", "skipped", "cancelled"), ("success", "failure"), (False, True)):
            expression = (condition.replace("!cancelled()", repr(not cancelled))
                          .replace("success()", repr(preceding == "success"))
                          .replace("failure()", repr(preceding == "failure"))
                          .replace("steps.browser_counter_comparison.outcome", repr(primary))
                          .replace("&&", "and").replace("||", "or"))
            actual = eval(expression, {"__builtins__": {}})
            self.assertEqual(actual, not cancelled and primary == preceding,
                             (primary, preceding, cancelled))

    def test_missing_duplicate_moved_or_weakened_controls_are_rejected(self):
        mutants = [self.source.replace(observation.IDENTIFIED_PRIMARY, observation.PRIMARY, 1),
                   self.source.replace(observation.CONDITION, "", 1),
                   self.source.replace(observation.CONDITION, observation.CONDITION * 2, 1),
                   self.source.replace("        id: browser_counter_comparison\n", "        id: foreign_primary\n", 1),
                   self.source.replace(observation.CONDITION, "        if: always()\n", 1),
                   self.source.replace(observation.CONDITION, observation.CONDITION.replace("!cancelled() && ", ""), 1),
                   self.source.replace(observation.CONDITION, observation.CONDITION.replace("== 'failure'", "!= 'success'"), 1),
                   self.source.replace(observation.IDENTIFIED_PRIMARY, observation.IDENTIFIED_PRIMARY + "        continue-on-error: true\n", 1),
                   self.source.replace(self.primary, self.primary + self.primary, 1),
                   self.source.replace(self.capture, self.capture + self.capture, 1)]
        moved = self.source.replace(observation.CONDITION, "", 1)
        package = step(moved, "Capture bounded Session deadline public tests", "nagi-package")
        mutants.append(moved.replace(package, package.replace(observation.CAPTURE, observation.REACHABLE_CAPTURE, 1), 1))
        for mutant in mutants:
            with self.subTest(sha256=hashlib.sha256(mutant.encode()).hexdigest()):
                with self.assertRaises(ValueError):
                    history.inverse_workflow(mutant)
        # Changing native commands, artifact/privacy limits or enforcement must
        # still fail the original frozen proof, never disappear in this inverse.
        for old, new in (("exit \"$profile_failed\"", "exit 0"),
                         ("limit=256*1024+1", "limit=512*1024+1"),
                         ('-- --exact', '-- --ignored'),
                         ('session-deadline-browser.raw"', 'unbounded.raw"'),
                         ("run: test '${{ steps.session_deadline_diag.outcome }}' = success", "run: true")):
            self.assertIn(old, self.source)
            mutant = self.source.replace(old, new, 1)
            try:
                restored = history.inverse_workflow(mutant)
            except ValueError:
                continue
            self.assertNotEqual(hashlib.sha256(restored.encode()).hexdigest(), BASE_WORKFLOW_SHA256)

    def test_real_primary_shell_keeps_cargo_and_inventory_failures_red(self):
        body = textwrap.dedent(self.primary.split("        run: |\n", 1)[1])
        fake = '''import json,os,sys
sys.path.insert(0, os.environ["OBSERVATION_CI"])
import sf07_checkpoint_summary as gate
args=sys.argv[1:]
with open(os.environ["OBSERVATION_CALLS"],"a") as stream: stream.write(json.dumps(args)+"\\n")
mode=os.environ["OBSERVATION_MODE"]
if "--no-run" in args:
 print("error[E0308]: public synthetic compile refusal" if mode=="compile" else "Finished test profile")
 sys.exit(101 if mode=="compile" else 0)
if "--list" in args:
 for name in gate.BROWSER_COUNTER_CASES: print(name+": test")
 print("10 tests, 0 benchmarks")
 sys.exit(101 if mode=="list" else 0)
names=next((tuple([name]) for name in gate.BROWSER_COUNTER_CASES if name in args), gate.BROWSER_COUNTER_CASES)
bad=mode=="inventory" and len(names)==1 and names[0]==gate.BROWSER_COUNTER_CASES[1]
print("running",len(names),"tests")
for name in names: print("test", "foreign_case" if bad else name, "... ok")
print("test result: ok.",len(names),"passed; 0 failed; 0 ignored; 0 measured;",10-len(names),"filtered out; finished in 0.01s")
sys.exit(101 if mode=="cargo" and len(names)==1 and names[0]==gate.BROWSER_COUNTER_CASES[1] else 0)
'''
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            script = directory / "fake_cargo.py"
            script.write_text(fake)
            for mode, expected, calls in (("success", 0, 5), ("cargo", 1, 5),
                                          ("inventory", 1, 5), ("compile", 1, 1), ("list", 1, 2)):
                call_log = directory / (mode + ".jsonl")
                env = {**os.environ, "OBSERVATION_CI": str(ROOT / "scripts/ci"),
                       "OBSERVATION_CALLS": str(call_log), "OBSERVATION_MODE": mode}
                shell = "cargo() { " + shlex.quote(sys.executable) + " " + shlex.quote(str(script)) + ' "$@"; }\n' + body
                result = subprocess.run(["bash", "-e", "-o", "pipefail", "-c", shell],
                                        cwd=ROOT, env=env, capture_output=True, timeout=15)
                self.assertEqual(result.returncode, expected, (mode, result.stdout, result.stderr))
                self.assertEqual(len(call_log.read_text().splitlines()), calls, mode)
                if calls == 5:
                    self.assertIn(b"browser_counter_probe profile=parallel-ten", result.stdout)
                if mode == "cargo":
                    self.assertIn(b"profile=single-strict cargo_exit=101 inventory_exit=0", result.stdout)
                if mode == "inventory":
                    self.assertIn(b"profile=single-strict cargo_exit=0 inventory_exit=1", result.stdout)

    def test_inherited_capture_executes_formatter_without_exposing_raw_failure(self):
        body = textwrap.dedent(self.capture.split("        run: |\n", 1)[1])
        fake = '''import os,sys
from pathlib import Path
root=Path(os.environ["OBSERVATION_ROOT"])
sys.path[:0]=[str(root/"scripts/ci"),str(root/"scripts/tests")]
import session_deadline_diagnostic as diag
import session_native_finish_summary as native
from test_session_deadline_diagnostic import cargo_log
args=sys.argv[1:]
failed=()
markers={}
if "--test" in args:
 browser="browser_login_public_api" in args
 names=diag.BROWSER_CASES if browser else diag.SESSION_CASES
 if browser and os.environ["OBSERVATION_FAIL"]=="1":
  failed=("input_and_strict_json_rejections_do_not_consume_or_call_verifier",)
  markers={failed[0]:["browser_login_phase_diag request=1 status=504 phase=16 event=1 before_gate=1 poll_gate=1 saw_pending=1 pending_phase=16 pending_gate=1 saw_live_pending=1 live_pending_phase=16 gate_refused=0 row_missing=0 rollback_ack=0 restore_commit_ack=0 exit=8 exit_gate=2 consumed=1"]}
elif "native_finish" in args: names=native.REQUIRED
else: names=(next(name for name in diag.DELIVERY_CASES.values() if name in args),)
sys.stdout.buffer.write(cargo_log(names,failed=failed,markers=markers))
sys.exit(101 if failed else 0)
'''
        with tempfile.TemporaryDirectory() as directory:
            directory = Path(directory)
            script = directory / "fake_cargo.py"
            script.write_text(fake)
            shell = "cargo() { " + shlex.quote(sys.executable) + " " + shlex.quote(str(script)) + ' "$@"; }\n' + body
            for fail in (False, True):
                owned = directory / str(fail)
                owned.mkdir()
                env = {**os.environ, "RUNNER_TEMP": str(owned), "OBSERVATION_ROOT": str(ROOT),
                       "OBSERVATION_FAIL": str(int(fail))}
                result = subprocess.run(["bash", "-e", "-o", "pipefail", "-c", shell],
                                        cwd=ROOT, env=env, capture_output=True, timeout=30)
                self.assertEqual(result.returncode, int(fail), (result.stdout, result.stderr))
                record = json.loads((owned / "session-deadline-diagnostic.json").read_text())
                self.assertEqual(record["status"], "tests_failed" if fail else "passed")
                if fail:
                    facts = record["suites"]["browser"]["browser_phase"]
                    self.assertEqual([(fact["phase"], fact["exit"]) for fact in facts], [(16, 8)])
                public = result.stdout + result.stderr + (owned / "session-deadline-diagnostic.json").read_bytes()
                self.assertNotIn(b"PRIVATE_SENTINEL", public)
                self.assertNotIn(b"/private/path", public)
                self.assertFalse(list(owned.glob("*.raw")))


if __name__ == "__main__":
    unittest.main()
