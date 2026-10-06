# 独立終了observerを使うSQLite adapterの概測

2026-10-06、Linux x86_64 VM、Rust test/debug profile、warm target。private direct Driverとdeadpool adapterの同じnative core・memory DB・SQL・row decode・rollbackを比較した。今回はcap1の直列測定で、多接続のthroughput比較ではない。

各backendで8回warmup後、32 batch×128 Tx、4096 sampleずつ。batchの先行順を交互にし、begin→`SELECT 1 AS n`→row decode→rollback返信を測定した。openとthread起動は測定区間に含めない。最後にnative close/actual joinをassertした。

| backend | p50 ns | p95 ns |
|---|---:|---:|
| direct | 65629 | 138899 |
| deadpool | 113731 | 159469 |

[summary.json](summary.json)にsource、toolchain、host、hashを保存し、[samples.csv](samples.csv)に8192 sampleを残した。percentileは昇順sampleの`(n-1)*percentile/100`の整数切捨てindex。CSVから元JSONと同じ値になることを確認した。

この一条件ではadapter側の待ち時間が大きかった。因果的overheadの上限や統計的な差の証明ではなく、高速化の根拠にも使わない。release profile、cold起動、多接続/競合、throughput、allocation、Future size、memory、binary/compile timeは未測定。過去の別source・並列試験中の数値と比較して性能改善を主張しない。

構造としてはnative worker＋独立observerの2thread/connection。ready reply用のArc/Mutexはconnection生成時に使う。warm直列sampleからthread・allocationの費用は判断できない。公開runtimeやcompilerにはこのcfg(test) prototypeを配線していない。

## 再実行と検証

```sh
NAGI_SQLITE_MEASURE_OUTPUT=/tmp/sqlite-independent-observer.json \
  cargo test --locked -p nagi-runtime \
  sqlite_prototype::comparison::direct_and_deadpool_warm_native_session_comparison \
  -- --exact --nocapture
```

[verification.json](verification.json)は実コマンド・exit・時間・source・原ログhashの記録。[audit](audit/)にはcompile RED、順次joinの実行時RED、50件のGREEN、runtime全体、clippy、fuzz、単独測定を置いた。panicを注入する既存回帰のログを実行失敗と取り違えず、各コマンドのexitとtest resultで判定する。全workspaceは92 suite/881成功、failed/ignored 0。

観測時のcommit IDはローカルの記録。GitHub経由のcommit生成でIDが異なっても、source treeと実装bytesの一致を確認する。初期compile REDは未実装APIによる失敗で、runtime REDや安全性の証明とは別。[設計と保証範囲](../../../docs/internal/sqlite-multiconnection-results.md)を参照。
