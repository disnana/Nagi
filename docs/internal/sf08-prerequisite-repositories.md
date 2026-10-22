# SF08 prerequisites: pinned Ubuntu repositories

2026-10-22, base `07a61144e3562efa6ba6a63c0522cdcaab145da8`, PR #149.

CI prerequisite failures reached the shared 240-second deadline, reported `apt_install`, observed actual ECHILD, and accepted zero browser executions. That stage label also covers refusal before install admission; the existing record did not establish whether install spawned or how long update consumed. The unavailable CI APT output does not establish a network or repository root cause.

## Adopted change

The pinned Playwright image contains both Ubuntu and NodeSource source configurations. The three required packages (`libnss3-tools`, `build-essential`, `pkg-config`) are absent initially and come from Ubuntu. Prerequisites now select only `/etc/apt/sources.list.d/ubuntu.sources` with explicit `Dir::Etc::sourcelist` and `Dir::Etc::sourceparts=-` options on **both** update and install. No configuration file is deleted or changed. Before each spawn, the file must be a bounded regular file, with no final-component symlink, and SHA-256 `0af89eff53f98803ea0d7088c5a1dd39a606c5e892e732ce60d6512e4e99a420`. Its parent directories are part of the fixed image. Missing, changed, special, or oversized files fail closed without falling back to other sources. `ubuntu.sources.fixture` preserves the public image bytes for host unit tests; its digest is checked independently.

TLS and package signatures, update Error-Mode=any, zero retries, 30-second transports, the single 240-second budget, all packages, and the existing process ownership capsule remain unchanged. Source checking consumes the original budget and cancellation is checked again before spawning. Browser TLS and business checks remain mandatory.

Failed prerequisites may additionally publish only three scalars after closed ownership **and** actual ECHILD: `update_elapsed_ms` (integer 0..240000), `install_spawned` (boolean), and `install_admission_remaining_ms` (integer 0..240000). The validator rejects unknown/missing keys and invalid types/ranges, including boolean-as-integer. Elapsed milliseconds are clamped observations, **not** precise measurements beyond the deadline. Remaining budget is recorded after source checking, just before attempted install spawn; `install_spawned` becomes true only after the capsule returns the leader. Raw APT output, URLs, hosts, certificates, and arbitrary error strings are never added to public diagnostics.

## Evidence and limits

Private evidence: `local-only/sf08-prerequisites/` in the original workspace, excluded from Git.

- Original all-repository private diagnostic: update exited 100 after 8.753 seconds with a TLS diagnostic. An APT-command-only public transport CA override separated that local managed-proxy setup issue; the next update exited 100 after 2.013 seconds because the unrelated NodeSource repository had no Release file. This is a local finding, **not** proof of the GitHub 240-second failure's root cause. No diagnostic CA option enters production.
- Ubuntu-only private diagnostic with fresh APT lists and that explicitly recorded APT-only CA override: update 7.896 seconds, install 20.626 seconds, all required tools present, closed ownership and actual ECHILD. This demonstrates removal of an unnecessary repository dependency; it is not an accepted browser run.
- The production prerequisite function then passed without a CA override in the existing diagnostic container (3.519 seconds, packages already installed).
- One fresh container then executed the unchanged full entrypoint with read-only source/Rust mounts, no host CA or managed CA override, fresh HOME/Cargo target/npm state. APT passed; Cargo build exited 101, diagnostic `unknown`, after an overall 76.045 seconds. Closure observed actual ECHILD; browsers accepted **0**. The build cause remains unknown; no trust-relaxed rerun was used. **Actual three-engine TLS/business acceptance remains required on the new-head CI.**

Regression evidence: the new source-selection assertion fails against the original prerequisite implementation (`source-selection-red.log`), then passes with the fix. Python integration/ownership suites: **64 passed** (`python-final.log`); Node protocol/TLS-proof tests: **148 passed** (`node-network-green.log`); source/projection guards: **16 passed**; CI policy: **160 passed**. The first Node invocation omitted socket/network sandbox permission and failed at the file-runner level; its original log is retained. The explicit network-permission run passed without JavaScript changes. The first Python run exposed an incomplete Mock drain setup in one new test, corrected without changing production behavior or existing expectations; its original log is retained.

Tests retain existing command/package/deadline/failure assertions and add both-command source selection, pinned-file rejection (missing/changed/directory/symlink/FIFO/oversized), pin recheck, cancellation/expiry during source checking, spent budget, install spawn failures, strict scalar validation, and independent closure/ECHILD rejection. Independent final High audit and exact-head CI are subsequent gates; this document does not claim either has completed.
