# 今回のコマンドと保証範囲

Linux x86_64、toolchainの完全版は`toolchain.txt`。`PATH`は`/workspace/toolchains/cargo/bin`、`CARGO_HOME=/workspace/toolchains/cargo`、`RUSTUP_HOME=/workspace/toolchains/rustup`。既存Cargo cacheは`/tmp/nagi-container-flow-target`、native cacheは`/tmp/nagi-task-native-target`。`CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2 CARGO_NET_OFFLINE=true`。環境固有pathをrepository既定に固定していない。socket検査にはnetwork権限を付けた。

以下はすべてexit 0。最終行まで取得した原ログを`logs/`へ保存した。費用oracle以外のignored/filterを実行成功として数えない。

```sh
cargo test --locked -p nagic --test frontend_contracts
python scripts/verify_task_handle_contract_inputs.py
cargo test --locked -p nagic --example task-contract-red
cargo run --locked -p nagic --example task-contract-red -- --report /workspace/nagi-task-post94-2026-10-07/task-contracts.json
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo run --locked -p nagic --example fuzz-smoke
python scripts/verify_compiler_contracts.py
python scripts/verify_library_examples.py --compiler /tmp/nagi-container-flow-target/debug/nagic
python scripts/verify_application_examples.py --compiler /tmp/nagi-container-flow-target/debug/nagic
/tmp/nagi-static-site-venv/bin/python benchmarks/results/task-spawn-post94-2026-10-07/verify-markdown.py /workspace/nagi-task-post94-2026-10-07/markdown.json
/tmp/nagi-static-site-venv/bin/python website/build.py --repository https://github.com/disnana/Nagi
git diff --check
```

測定packageは今回のcheck済みHigh→保存Low→finalize→Rustと、同保証の手書きRustから準備した。release buildは既存native targetを共有し、build logは同条件の時間比較やclean buildの根拠ではない。生成/手書きbinaryは順番に実行したが、hostでは全回帰/buildも進行していた。

```sh
cargo run --locked -p nagic --example task-handles-cost -- /tmp/nagi-task-post94-cost
CARGO_TARGET_DIR=/tmp/nagi-task-native-target cargo build --locked --offline --release --manifest-path /tmp/nagi-task-post94-cost/generated/Cargo.toml
CARGO_TARGET_DIR=/tmp/nagi-task-native-target cargo build --locked --offline --release --manifest-path /tmp/nagi-task-post94-cost/manual/Cargo.toml
NAGI_BENCH_LOOPS=25 /tmp/nagi-task-native-target/release/task-cost-generated
NAGI_BENCH_LOOPS=25 /tmp/nagi-task-native-target/release/task-cost-manual
cargo test --locked --release -p nagi-runtime task::cost_tests::native_task_handle_costs -- --ignored --nocapture
```

比較集計時に7 JSONL行×2を読み、batch Future metadataと6条件のallocation全項目の一致、各7反復をassertした。原timeは変更せず保存した。runtime費用oracleは`cfg(test)`の別workload/layoutであり、1群成功と他testのfilterを分けた。

独立Sol Highはread-onlyのsource/diffレビューで、今回のテストを再実行していない。4 OS/IDE/archive gateは今回PRのGitHub Actionsで別に確認し、その実行URL・結論はPR説明とChecksに読戻して記録する。
