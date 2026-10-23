"""Exact inverse for post-exit stream framing; native commands stay unchanged."""

PROFILES = (
    ("csrf", "--cfg nagi_auth_stage_diag",
     "auth::csrf::tests::same_queue_reopen_reuses_data_context_without_refreshing_token_expiry -- --exact --nocapture"),
    ("prelogin", "--cfg nagi_auth_stage_diag",
     "auth::prelogin::tests::consumed_bearer_identity_can_issue_session_without_replacing_original_ceiling -- --exact --nocapture"),
    ("broad", "--cfg nagi_auth_stage_diag --cfg nagi_browser_login_diag", "-- --nocapture"),
    ("cases", "--cfg nagi_auth_stage_diag --cfg nagi_browser_login_diag", "-- --list"),
)
UNIT = "          python -B scripts/tests/test_auth_stage_join.py\n"
START = "          python scripts/tests/test_auth_stage_diagnostic.py\n          set +e\n"
INITIALIZE = START + "          join_failed=0\n"
FAILURE = '          if [ "$stage_unit_rc" -ne 0 ] || [ "$stage_rc" -ne 0 ]; then sanitize_rc=1; fi\n'
JOIN_FAILURE = FAILURE.replace('; then', ' || [ "$join_failed" -ne 0 ]; then')
CLEANUP = ('          rm -f "$RUNNER_TEMP/auth-stage-csrf.raw" "$RUNNER_TEMP/auth-stage-prelogin.raw" '
           '"$RUNNER_TEMP/auth-stage-broad.raw" "$RUNNER_TEMP/auth-stage-cases.raw"\n')
JOIN_CLEANUP = CLEANUP.rstrip("\n") + "".join(
    f' "$RUNNER_TEMP/auth-stage-{name}.stderr.raw"' for name, _, _ in PROFILES) + "\n"


def commands(name, flags, selection):
    old = (f"          RUSTFLAGS='{flags}' cargo test --locked -p nagi-runtime --lib {selection} "
           f'> "$RUNNER_TEMP/auth-stage-{name}.raw" 2>&1\n'
           f'          {name}_rc=$?\n')
    new = old.replace(' 2>&1\n', f' 2> "$RUNNER_TEMP/auth-stage-{name}.stderr.raw"\n')
    new += f'          python -B scripts/ci/auth_stage_join.py "$RUNNER_TEMP" {name} || join_failed=1\n'
    return old, new


def edits():
    return [(START, UNIT + INITIALIZE), (FAILURE, JOIN_FAILURE), (CLEANUP, JOIN_CLEANUP),
            *(commands(*profile) for profile in PROFILES)]


def inverse(source):
    for old, new in edits():
        if source.count(new) != 2:
            raise ValueError("auth-stage stream registration mismatch")
        source = source.replace(new, old)
    return source
