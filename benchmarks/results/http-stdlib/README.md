# Initial standard HTTP measurements

Collected on 2026-10-03 using `scripts/http_stdlib_bench.py` and the HTTP change based on main `d958dba`.

This first comparison used the workspace Rust example binary. Its shared dependency lock differed from the generated Nagi application's lock: libc 0.2.189 versus 0.2.190. The initial POST throughput median was Nagi 59,279 requests/s versus Rust 66,489 requests/s. These records are retained rather than treated as a controlled language comparison. The dependency difference has not been established as the cause of that gap.

The [matched comparison](../http-stdlib-matched/) uses the generated application's dependency lock and release configuration for both implementations. Its repeated POST measurements did not reproduce the initial deficit.

`runs.json` contains all 27 five-second measurements; `summary.json` contains medians of three repeats. `wrk-*.txt` retains client output, including socket and status errors. `environment.json` records affinity, container limits, CPU model, and executable hashes. The legacy Axum server has a different admission/deadline policy and is a separate reference.

`soak.json` and `soak-wrk.txt` contain a 120-second Nagi POST run, including one-second RSS/CPU/FD samples and five seconds of observation after traffic stopped. That Nagi executable is identical to the one in the matched comparison. The run completed 7,653,211 requests without transport/status errors; FDs returned from 74 to the original 10 while the process remained running. Two minutes is not long-term operational validation.
