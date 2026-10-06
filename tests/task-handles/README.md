# Task結果handleの段階別契約入力

S1作業branchのHigh/Low契約。`contracts.json`はchecker成功、またはchecker段階＋diagnostic fragment＋primary元行の拒否を指定する。parse/import拒否・ICE・違う行の拒否はnegativeの成功ではない。登録検査とrunner oracleだけではTaskの成功を保証しない。

```sh
python scripts/verify_task_handle_contract_inputs.py
cargo test --locked -p nagic --example task-contract-red
cargo run --locked -p nagic --example task-contract-red -- --report /tmp/task-contracts.json
cargo test --locked -p nagic --test task_handles
```

元の28対にbinding放棄、branch合流/再生成、nested/same-depth scope、引数/return/wrapper escape、Failure viewとClone/shared境界、ユーザーspawn名互換の縮小反例を追加。Task native harnessは全positiveをHigh・保存Low・手書きLowでbuild/runし、別のbarrier付きoracleで業務Err、sticky fault、body/legacy元Err、Dropとactual joinを区別する。固定seedの16経路も両flagで三構文実行する。private試作からpublicへの17runtime oracleは `task::tests::`。

[Stage 1結果](../../docs/internal/task-bridge-stage1-results.md)は50入力がparse REDだった歴史的記録。[接続結果](../../docs/internal/task-handles-s1-results.md)と[新artifact](../../benchmarks/results/task-handles-s1-2026-10-06/README.md)に今回の実行・原ログ・hash・保証範囲を残す。S2とSQLiteは対象外。
