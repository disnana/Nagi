"""Reach the existing closed diagnostic after its own primary Browser failure."""
import re

PRIMARY = "      - name: Compare BrowserLogin failure cases alone and with the parallel public suite\n"
IDENTIFIED_PRIMARY = PRIMARY + "        id: browser_counter_comparison\n"
CAPTURE = ("      - name: Capture bounded Session deadline public tests\n"
           "        id: session_deadline_diag\n")
CONDITION = ("        if: >-\n"
             "          !cancelled() && ((success() && steps.browser_counter_comparison.outcome == 'success') ||\n"
             "          (failure() && steps.browser_counter_comparison.outcome == 'failure'))\n")
REACHABLE_CAPTURE = CAPTURE + CONDITION


def inverse_workflow(source):
    job = re.search(r"(?ms)^  linux:\n.*?(?=^  [a-z][a-z0-9-]*:|\Z)", source)
    if job is None:
        raise ValueError("Browser primary Linux job missing")
    block = job.group()
    for header in (IDENTIFIED_PRIMARY, REACHABLE_CAPTURE):
        if block.count(header) != 1 or source.count(header) != 1:
            raise ValueError("Browser failure observation placement changed")
    # These are control metadata only. The entire inherited primary/capture
    # bodies, artifact paths, enforcement and frozen workflow hash remain intact.
    if source.count("        id: browser_counter_comparison\n") != 1 or source.count(CONDITION) != 1:
        raise ValueError("Browser failure observation duplicated or changed")
    primary = re.search(re.escape(IDENTIFIED_PRIMARY) + r".*?(?=^      - |\Z)", block, re.M | re.S)
    capture = re.search(re.escape(REACHABLE_CAPTURE) + r".*?(?=^      - |\Z)", block, re.M | re.S)
    if (primary is None or capture is None or primary.start() >= capture.start()
            or primary.group().split("        run: |\n", 1)[0] != IDENTIFIED_PRIMARY + "        shell: bash\n"
            or capture.group().split("        run: |\n", 1)[0] != REACHABLE_CAPTURE + "        continue-on-error: true\n        shell: bash\n"):
        raise ValueError("Browser failure observation control changed")
    return source.replace(IDENTIFIED_PRIMARY, PRIMARY, 1).replace(REACHABLE_CAPTURE, CAPTURE, 1)
