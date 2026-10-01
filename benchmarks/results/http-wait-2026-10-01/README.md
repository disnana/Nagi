# HTTP request-wait deadline measurements

The Japanese and English reports are in [docs/http-capacity.md](../../../docs/http-capacity.md) and [docs/en/http-capacity.md](../../../docs/en/http-capacity.md). Measurements ran on October 1, 2026 (UTC), over loopback only.

- `lifecycle/`: 100 connections each for silence, partial headers, partial bodies, and unused keep-alive; normal and abrupt disconnect recovery.
- `baseline-saturation/`, `candidate-saturation/`: the initial sequential before/after comparison, three five-second trials per endpoint.
- `paired/`: the alternating before/after comparison used in the report, three five-second trials per implementation and endpoint. Each trial retains wrk output.
- `recovery-load/`: 1,050 waiting connections during 25 seconds of paced healthy traffic, including FD/RSS samples. `initial-attempt.json` preserves the first response aggregate and explains the corrected arrival-count assertion.

`provenance.json` identifies the source files used to build the modified runtime. Original `source_commit` fields identify the checked-out base commit, not a clean checkout containing the candidate changes. Binary and harness hashes are retained; binaries and Vegeta's binary response files are omitted.

To repeat the alternating comparison, build `examples/crud.nagi` at the recorded baseline commit and with the candidate runtime, using distinct native target directories. Run `paired/harness.py` with `--repository`, `--baseline`, `--candidate`, `--wrk`, and a new `--out` directory. `recovery-load/harness.py` accepts `--binary`, `--vegeta`, and a new `--out` directory. Both use separate available CPUs for the server and clients. The existing scripts in `scripts/` reproduce the lifecycle and initial saturation tests; their measured snapshots are retained here.

All saturation tests use 128 **client** connections, one server CPU/worker, and two client CPUs/threads. This is not a server connection cap. The results do not establish public-network capacity, DDoS resistance, or stability over a full day.
