# SF01 bounded cost fixture

This fixture compares a generated Nagi authorization policy with a hand-written Rust policy through the same public `std.http.server` API. Both variants use `authorized_policy`, the standard request verifier and `AuthScope` binding, the same policy conditions, `Grant::from_authorized`, a one-slot bounded reservation, and `Grant.submit` for the same target-bound in-memory document read. The only intentional difference is where the policy condition is written.

The verifier, authorizer adapter, and in-memory native operation are trusted callbacks. The verifier accepts only the fixture credential `Bearer sf01-fixture`, creates a short-lived `VerifiedIdentity`, and never acts as a production authentication service. No test-only proof issuer, legacy API, or disabled security mode is used.

Run from the repository root with the toolchain used for the recorded result:

```sh
CARGO_HOME=/workspace/toolchains/cargo \
RUSTUP_HOME=/workspace/toolchains/rustup \
CARGO_TARGET_DIR=/workspace/nagi-sf01-target \
NAGI_NATIVE_TARGET_DIR=/workspace/nagi-sf01-native-target \
CARGO_NET_OFFLINE=true \
CARGO_INCREMENTAL=0 \
CARGO_BUILD_JOBS=2 \
CARGO_PROFILE_DEV_DEBUG=0 \
CARGO_PROFILE_TEST_DEBUG=0 \
CARGO_PROFILE_RELEASE_DEBUG=0 \
NAGI_THREADS=1 \
python3 benchmarks/security-sf01/run_security_sf01.py \
  --compiler /workspace/nagi-sf01-target/debug/nagic \
  --output benchmarks/results/security-sf01-2026-10-08
```

The runner refuses to overwrite a completed `measurement.json`. It performs compiler checks and three release builds per variant, then uses at most 64 sequential requests over an owned loopback fixture. It records build and server logs alongside source hashes and raw samples. See the result directory README for the measured conditions and interpretation limits.
