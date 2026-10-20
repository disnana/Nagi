"""Closed native-finish numeric grammar. Never renders rejected input."""
import re

PREFIX = "session_native_finish_diag"
LIMIT_US = 60_000_000
UNSEEN_US = LIMIT_US + 1
FIELDS = ("status", "origin", "terminal", "diagnostic_valid", "native_state", "commit_result",
          "cleanup_result", "cleanup_reason", "commit_enter_us", "commit_return_us", "cleanup_enter_us",
          "cleanup_return_us", "settled_us", "begins", "commits", "selected_deadline_passed")
TIMES = ("commit_enter_us", "commit_return_us", "cleanup_enter_us", "cleanup_return_us", "settled_us")
MARKER = re.compile(PREFIX + " " + " ".join(re.escape(name) + r"=(0|[1-9][0-9]{0,7})"
                                            for name in FIELDS) + r"\Z")


def valid(values):
    if type(values) is not dict or set(values) != set(FIELDS) or any(type(v) is not int for v in values.values()):
        return False
    v = values
    if (v["status"] != 504 or v["origin"] not in range(4) or v["diagnostic_valid"] != 1
            or v["native_state"] not in range(8) or v["commit_result"] not in range(3)
            or v["cleanup_result"] not in range(4) or v["cleanup_reason"] not in range(4)
            or v["selected_deadline_passed"] not in (0, 1)
            or not 0 <= v["commits"] <= v["begins"] <= 1023
            or any(not 0 <= v[key] <= UNSEEN_US for key in TIMES)):
        return False
    if v["origin"] in (0, 2) and v["terminal"] != 0:
        return False
    if v["origin"] == 1 and v["terminal"] not in range(1, 7):
        return False
    if v["origin"] == 3 and v["terminal"] not in range(1, 5):
        return False
    ce, cr, ee, er, settled = (v[key] for key in TIMES)
    commit, cleanup, reason, state = (v[key] for key in ("commit_result", "cleanup_result", "cleanup_reason", "native_state"))
    if commit == 0:
        if cr != UNSEEN_US:
            return False
    elif ce == UNSEEN_US or cr == UNSEEN_US or cr < ce:
        return False
    if cleanup == 0:
        if reason != 0 or ee != UNSEEN_US or er != UNSEEN_US:
            return False
    else:
        if reason == 0 or ee == UNSEEN_US:
            return False
        if cleanup == 1 and er != UNSEEN_US:
            return False
        if cleanup in (2, 3) and (er == UNSEEN_US or er < ee):
            return False
        if ce != UNSEEN_US and ee < ce or cr != UNSEEN_US and ee < cr:
            return False
        if reason == 1 and ce != UNSEEN_US or reason == 2 and commit != 2 or reason == 3 and commit == 2:
            return False
    if state == 0 and (ce != UNSEEN_US or commit != 0 or cleanup != 0):
        return False
    if state == 1 and (ce == UNSEEN_US or commit != 0 or cleanup != 0):
        return False
    if state in (2, 3) and (commit != state - 1 or cleanup != 0):
        return False
    if state in (4, 5, 6) and cleanup != state - 3:
        return False
    if state == 7:
        if settled == UNSEEN_US or cleanup == 1 or any(t != UNSEEN_US and t > settled for t in (ce, cr, ee, er)):
            return False
    elif settled != UNSEEN_US:
        return False
    if v["begins"] == 0 and (state != 0 or v["commits"] != 0):
        return False
    return True


def parse(line):
    match = MARKER.fullmatch(line)
    if match is None:
        return None
    values = dict(zip(FIELDS, map(int, match.groups())))
    return values if valid(values) else None


def paired(values, legacy, origin):
    """Legacy was parsed independently; never infer request identity from time."""
    if not valid(values) or type(legacy) is not dict or values["origin"] != origin:
        return False
    expected_terminal = legacy.get("terminal", 0) if origin in (1, 3) else 0
    if (legacy.get("status") != 504 or values["terminal"] != expected_terminal
            or any(values[key] != legacy.get(key) for key in ("begins", "commits"))):
        return False
    # A Timer branch / integer-ms zero is not proof that the selected std
    # deadline has passed. Preserve either sampled boolean for diagnosis.
    return True
