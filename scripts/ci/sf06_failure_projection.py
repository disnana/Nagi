"""Exact reversible SF06 observation hook; the inherited guard stays byte-identical."""
import re

HEADER = "      - name: Check SF06 public HTTP client across package targets\n"
WITH_ID = HEADER + "        id: sf06_public_client\n"
ORIGINAL = '''          run_check() {
            local label="$1"
            shift
            local log="$RUNNER_TEMP/sf06-${label}.log"
            local status=0
            "$@" > "$log" 2>&1 || status=$?
            python "$RUNNER_TEMP/sf06-http-client-guard.py" "$label" "$status" "$log"
          }
'''
OBSERVED = '''          run_check() {
            local label="$1"
            shift
            local log="$RUNNER_TEMP/sf06-${label}.log"
            local status=0
            (unset GITHUB_OUTPUT; "$@") > "$log" 2>&1 || status=$?
            local guard_status=0
            (unset GITHUB_OUTPUT; python "$RUNNER_TEMP/sf06-http-client-guard.py" "$label" "$status" "$log") || guard_status=$?
            if [ "$guard_status" -ne 0 ]; then
              python -B scripts/ci/sf06_http_client_failure_diagnostic.py "$label" "$status" "$guard_status" "$log" "$RUNNER_TEMP" 2>/dev/null || printf '%s\\n' 'sf06_http_client_failure_diag=v1 status=unavailable reason=invocation_error artifact_saved=false'
            fi
            return "$guard_status"
          }
'''
ARTIFACT = "          path: ${{ runner.temp }}/sf06-http-client-native/sf06-native-diagnostic.json\n"
OBSERVED_ARTIFACT = ("          path: |\n"
                     "            ${{ runner.temp }}/sf06-http-client-native/sf06-native-diagnostic.json\n"
                     "            ${{ steps.sf06_public_client.outputs.closed_failure_diagnostic }}\n")
PRIVATE_TRANSPORT_CURRENT = '"sf06-private-transport.log": 83'
PRIVATE_TRANSPORT_INHERITED = '"sf06-private-transport.log": 82'
PUBLIC_RUNTIME_CURRENT = '"runtime-http-client": (222, None, RUNTIME_CASES),'
PUBLIC_RUNTIME_INHERITED = '"runtime-http-client": (221, None, RUNTIME_CASES),'
Q1_PORT_PAIR_LINE = (
    "              'http_client::address::socket_pin::q1_native::tests::"
    "q1_native_port_pair_retry_releases_guard_and_is_bounded',\n"
)


def inverse_port_pair_inventory(source, *, allow_inherited=False):
    """Restore only the reviewed SF06 port-pair test inventory delta."""
    current = (source.count(PRIVATE_TRANSPORT_CURRENT) == 2
               and source.count(PRIVATE_TRANSPORT_INHERITED) == 0
               and source.count(PUBLIC_RUNTIME_CURRENT) == 1
               and source.count(PUBLIC_RUNTIME_INHERITED) == 0
               and source.count(Q1_PORT_PAIR_LINE) == 2)
    inherited = (source.count(PRIVATE_TRANSPORT_CURRENT) == 0
                 and source.count(PRIVATE_TRANSPORT_INHERITED) == 2
                 and source.count(PUBLIC_RUNTIME_CURRENT) == 0
                 and source.count(PUBLIC_RUNTIME_INHERITED) == 1
                 and source.count(Q1_PORT_PAIR_LINE) == 0)
    if allow_inherited and inherited:
        return source
    if not current:
        raise ValueError('SF06 port-pair inventory mismatch')
    source = source.replace(PRIVATE_TRANSPORT_CURRENT, PRIVATE_TRANSPORT_INHERITED)
    source = source.replace(PUBLIC_RUNTIME_CURRENT, PUBLIC_RUNTIME_INHERITED)
    return source.replace(Q1_PORT_PAIR_LINE, '')


def inverse_workflow(source):
    job = re.search(r"(?ms)^  nagi-package:\n.*?(?=^  [a-z][a-z0-9-]*:|\Z)", source)
    if job is None:
        raise ValueError("SF06 package job missing")
    block = job.group()
    step = re.search(r"(?ms)^      - name: Check SF06 public HTTP client across package targets\n.*?(?=^      - |\Z)", block)
    artifact = re.search(r"(?ms)^      - name: Preserve closed SF06 native diagnostics\n.*?(?=^      - |\Z)", block)
    if step is None or artifact is None:
        raise ValueError("SF06 diagnostic placement missing")
    if step.group().count(WITH_ID) != 1 or step.group().count(OBSERVED) != 1:
        raise ValueError("SF06 failed guard observation changed")
    if artifact.group().count(OBSERVED_ARTIFACT) != 1:
        raise ValueError("SF06 closed artifact binding changed")
    for new, old in ((WITH_ID, HEADER), (OBSERVED, ORIGINAL), (OBSERVED_ARTIFACT, ARTIFACT)):
        if source.count(new) != 1:
            raise ValueError("SF06 diagnostic registration missing or duplicated")
        source = source.replace(new, old, 1)
    return inverse_port_pair_inventory(source)
