# SF01 bounded cost evidence (2026-10-08)

The recorded run passed with 62 loopback requests, below the 64-request limit. It compares generated Nagi policy code and hand-written Rust policy code while keeping the standard `authorized_policy` dispatcher, verifier, policy conditions, grant creation, one-slot reservation, `Grant.submit` target check, handler, response bytes, and small request input the same. The only intended policy difference is generated Nagi versus hand-written Rust.

The recorded repository source head was `d127b28c292a6b58b401738ed061ff3461bcddba`; see `source-provenance.json` for the compiler and fixture hashes. Whether the working tree was clean at measurement time was not recorded.

Each variant received one explicit public `/health` readiness request, one valid warmup, 27 sequential valid keep-alive requests, one wrong-target denial, and one missing-credential denial. There was one server worker and at most one request at a time. The valid request was `GET /documents/1` with `Bearer sf01-fixture`, an empty body, and the fixed JSON response `{"id":1,"title":"authorized fixture"}`. Both denial controls produced the same expected standard-policy status/body; the missing-credential response also carried `WWW-Authenticate: Bearer`.

## Observed values

| Measurement | Generated Nagi policy | Hand-written Rust policy |
| --- | ---: | ---: |
| Release build wall time, median of 3 | 15.623 s | 7.499 s |
| Executable size | 1,524,160 bytes | 1,523,960 bytes |
| Sequential loopback latency median, 27 samples | 146,321 ns | 142,711 ns |
| Reported lower-rank p95 sample | 251,420 ns | 308,033 ns |
| Inner policy future size | 88 bytes | 80 bytes |
| Verifier / read future size | 256 / 288 bytes | 256 / 288 bytes |
| `VerifiedIdentity` / `AuthScope` / `Grant<Read>` layout | 24 / 16 / 24 bytes | 24 / 16 / 24 bytes |
| Synchronous `Grant::from_authorized` allocations | 0 | 0 |

These are measurements of these two instrumented fixture binaries on the recorded machine, not a general Nagi-versus-Rust performance result. The build wall times include generation, Cargo, and observed package-cache/build-directory lock waits; another workspace build ran concurrently. Raw per-build logs also retain Cargo's `Finished release` times (14.77 / 13.91 / 14.70 s for the Nagi fixture; 6.25 / 6.49 / 6.37 s for the Rust fixture). The shared target/cache was retained, the variants were built in Nagi-then-Rust order, and no clean target was used, so treat the compile values as evidence for this exact run rather than a controlled compiler speed comparison. Build artifacts had different generated paths and hashes. The runtime binaries include measurement instrumentation, and the short loopback latency samples show ordinary machine noise. Do not use the timing or size values as a production SLO, an end-to-end authentication benchmark, or an application throughput estimate.

The reported lower-rank p95 uses the zero-based index `floor((n - 1) * 0.95)` over sorted samples, as implemented by the runner.

## Post-measurement request-budget guard

The recorded run observed 62 total requests and retained that result without remeasurement. The runner used for that run had the SHA-256 recorded in `source-provenance.json`; its `maximum: 64` check happened after the request loop, so it documented the observed total but was not a pre-send hard cap. After measurement, the current runner was changed to check the aggregate request budget before every readiness, warmup, normal, and denial request. It permits request 64 and rejects request 65 before transmission. The current runner hash is recorded separately in `source-provenance.json`; it does not replace the measured runner hash. No original runner snapshot was retained or found, so the pre-change source is not reconstructed or presented as an artifact. No measurement was rerun after this guard change.

## Scope and limits

The callbacks are trusted native code. The verifier is a fixed local fixture verifier rather than a production credential service; the authorized operation is a bounded in-memory read rather than SQLite or a remote backend. The run exercises the real standard HTTP dispatcher and request-bound authorization path, including the public policy, grant and target-bound submission APIs.

The public API does not expose `AuthScope` lease heap allocation, reference counts, or the full request-to-drop lifetime. The measured scope interval begins at authorizer callback entry and ends when the grant is minted; dispatcher work before authorizer entry is excluded. Visible grant intervals run from grant mint to the native read adapter and to `Grant.submit`. The report also measures the callback's erased future handle size and the inner verifier, policy, and read futures, but it cannot inspect the private outer dispatcher future body. Allocation counts cover synchronous grant minting and allocations on the calling thread during selected `Future::poll` calls; they do not include future storage or allocations on other threads. These limitations are recorded in `measurement.json`.

This run does not benchmark streaming, concurrency scaling, capacity exhaustion, service startup under load, or application/database throughput. It makes no claim about unimplemented general streaming behavior.

## Artifacts

- `measurement.json`: request inputs, raw latency samples, policy outcomes, future/object layout, allocation windows, visible lifetime intervals, and explicit measurement limits.
- `build-results.json`: compiler check status, three release build wall times and executable sizes/hashes per variant.
- `environment.json`: toolchain versions, compiler hash, environment, cache/debug profile, and fixture source hashes.
- `source-provenance.json`: source HEAD and post-measurement executable cleanup record.
- `nagi-policy/` and `rust-policy/`: copied source, generated Low/Rust project artifacts, per-build/check logs, and server logs. The six built executable copies were removed after measurement to reduce artifact size; their file paths, measured sizes, and SHA-256 values remain in `build-results.json`.
- `runner.log`: final runner status and request count.

Repeated generated Rust diagnostic line maps (`.nagi/.../provenance.json`, containing only `rust_lines` and `schema_version`) are omitted after recording their original path, bytes, SHA-256, and entry count in `omitted-generated-line-maps.json`. Generated Rust/Low, original execution logs, measurement records, and source/environment provenance are retained. Archived generation directories are evidence, not runnable distributions.
