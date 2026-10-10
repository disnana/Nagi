# SF05 の境界検証

確定した[契約](../../docs/internal/security-foundation/sf05-contract.md)の RED、checker/native/runtime、移行アプリと小さい費用比較を分けて記録する。原ログと SHA-256 は `benchmarks/results/security-sf05-validation-2026-10-09/`、結果と未確認範囲は[結果記録](../../docs/internal/security-foundation-sf05-results.md)。過去の失敗を成功へ読み替えない。

以下は保存済み Linux 環境の再実行例。warm cache の再利用であり clean build とは呼ばない。別検証と同じ target を同時に使わず、出力には存在しない専用 directory を指定する。依存版や release/version を変更しない。

```sh
export PATH=/workspace/nagi-sf01-target/debug:/workspace/toolchains/cargo/bin:$PATH
export CARGO_HOME=/workspace/toolchains/cargo RUSTUP_HOME=/workspace/toolchains/rustup
export CARGO_TARGET_DIR=/workspace/nagi-sf01-target NAGI_NATIVE_TARGET_DIR=/workspace/nagi-sf01-native-target
export CARGO_NET_OFFLINE=true CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=2
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
python benchmarks/security-sf05/run_validation.py /workspace/sf05-scoped-check --stage compiler
python benchmarks/security-sf05/run_validation.py /workspace/sf05-runtime-check --stage runtime
python benchmarks/security-sf05/run_validation.py /workspace/sf05-engine-free --stage engine-free
python benchmarks/security-sf05/capture_command.py --marker '0 skipped' /workspace/sf05-editor -- python benchmarks/security-sf05/editor_checks.py
python benchmarks/security-sf05/capture_command.py --marker '84 business checks' /workspace/sf05-apps -- python benchmarks/security-sf05/http_examples.py
python benchmarks/security-sf05/capture_command.py --marker 'no performance threshold' /workspace/sf05-cost -- python benchmarks/security-sf05/cost_check.py --out /tmp/sf05-cost-new
```

native stage では `NAGI_TEST_ARTIFACT_DIR` を新しい専用 directory へ指定すると High・High 削除後の保存 Low・手書き Low の source、生成 Rust、build/run 原出力を保存できる。purpose guard は test の実行数を確認し、0 filtered や skip を成功に数えない。CI は四 OS で SF05 checker12/native1/admission6 をそれぞれ明示実行し、exact count/0ignored と source/log hash を artifact に残す。full SQLite79 と既存 lifecycle/authorizer/NULL/close 回帰も保持する。

`http_examples.py` は所有する小さい loopback DB に各 CRUD 一行だけを作り、High と元 High を消した保存 Low で実行する。SQL文字列の外部攻撃、負荷/枯渇、巨大 allocation は行わない。`cost_check.py` は既存 Query fixture の4 warmup/32 samples、generated/manual の同じ Options/Tx、rollback と actual close を使う。calling-thread allocation と未 poll Future サイズだけを比較し、worker/SQLite allocation、他 OS の費用や本番性能を推定しない。threshold を新設しない。
