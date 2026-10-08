#!/usr/bin/env bash
set -euo pipefail
export PATH="/workspace/toolchains/cargo/bin:$PATH"
export RUSTUP_HOME=/workspace/toolchains/rustup CARGO_HOME=/workspace/toolchains/cargo
export CARGO_TARGET_DIR=/workspace/nagi-build-cache/check NAGI_NATIVE_TARGET_DIR=/workspace/nagi-build-cache/native
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 CARGO_NET_OFFLINE=true
cd /workspace/Nagi-sqlite-public
nagi_evidence=/workspace/nagi-sqlite-public-2026-10-08
cargo fmt --all -- --check > "$nagi_evidence/workspace-fmt-final.log" 2>&1
cargo clippy --locked --all-targets -- -D warnings > "$nagi_evidence/workspace-clippy-final.log" 2>&1
cargo test --locked -p nagic --example sqlite-contract-check --example task-contract-red > "$nagi_evidence/runner-oracles-final.log" 2>&1
cargo run --locked -p nagic --example sqlite-contract-check -- --report "$nagi_evidence/contracts-final.json" > "$nagi_evidence/contracts-final.log" 2>&1
cargo run --locked -p nagic --example task-contract-red -- --report "$nagi_evidence/task-contracts-final.json" > "$nagi_evidence/task-contracts-final.log" 2>&1
cargo build --release --examples --bins --locked > "$nagi_evidence/release-build.log" 2>&1
python scripts/verify_sqlite_example.py > "$nagi_evidence/sqlite-example-native.log" 2>&1
cargo run --locked -p nagic --example fuzz-smoke > "$nagi_evidence/fuzz-final.log" 2>&1
python scripts/build_examples.py > "$nagi_evidence/examples-final.log" 2>&1
cargo test --locked -p nagic --no-default-features --test cli --test sql_check > "$nagi_evidence/no-sql-feature.log" 2>&1
python scripts/releases/package.py --binary "$CARGO_TARGET_DIR/release/nagic" --version 0.1.11 --platform linux-x86_64 --out build/sqlite-distribution > "$nagi_evidence/archive-package.log" 2>&1
python - <<'PY' > "$nagi_evidence/archive-verify.log" 2>&1
from pathlib import Path
import sys
sys.path.insert(0,'scripts/releases')
from verify import verify
verify(Path('build/sqlite-distribution/nagi-0.1.11-linux-x86_64.tar.gz'), '0.1.11', 'linux-x86_64', Path('/workspace/nagi-build-cache/native'))
PY
