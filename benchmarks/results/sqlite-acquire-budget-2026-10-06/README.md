# SQLite取得予算の検証と概測

2026-10-06、Linux x86_64 VM、Rust test/debug profile、warm target。private cap1、memory DB、同じnative coreで、begin→`SELECT 1 AS n`→decode→rollback返信を測定した。open／thread起動は区間外。各backendは8回warmup＋32 batch×128 Txで4096 sample、先行順を交互にした。native close／actual joinを最後にassertした。

| 同一実行のadapter | p50 µs | p95 µs | 未poll begin Future byte |
|---|---:|---:|---:|
| 従来のprivate無期限fixture | 92.219 | 149.365 | 2024 |
| 取得予算あり | 82.354 | 156.686 | 2040 |

Futureの差は16 byte。`size_of_val`の値で、heap allocationやNagi生成Futureの比較ではない。p50とp95の大小は逆で、この一条件から高速化・無負担・一般的なoverhead上限を主張しない。

既存direct比較も維持した。修正後の別実行ではdirect p50/p95 84.597/154.172µs、adapter 65.308/152.990µs。修正前の基点では61.793/132.039µsと60.290/122.404µsだった。VMの測定には変動があり、これを修正前後の性能改善とは解釈しない。release、多接続競合、throughput、allocation count、memory、binary／compile time、cold起動は未測定。

[summary.json](summary.json)に条件・source・hash・percentile、各CSVに生データを保存した。[budget-samples.csv](budget-samples.csv)が予算比較、[direct-samples.csv](direct-samples.csv)が修正後direct比較、[baseline-samples.csv](baseline-samples.csv)が基点。percentileのindexは昇順sampleの`(n-1)*percentile/100`を切り捨てる。各CSVの4096 sampleずつから値を再確認した。

## 再実行

```sh
NAGI_SQLITE_BUDGET_MEASURE_OUTPUT=/tmp/sqlite-budget.json \
  cargo test --locked -p nagi-runtime \
  sqlite_prototype::comparison::bounded_and_unbounded_deadpool_warm_comparison \
  -- --exact --nocapture

NAGI_SQLITE_MEASURE_OUTPUT=/tmp/sqlite-direct.json \
  cargo test --locked -p nagi-runtime \
  sqlite_prototype::comparison::direct_and_deadpool_warm_native_session_comparison \
  -- --exact --nocapture
```

[verification.json](verification.json)はcommand／exit／時間／原ログhash。[audit](audit/)には未実装compile RED、native予算なしの実行RED、60件GREEN、runtime、workspace、clippy、fuzz、単独測定を保存した。REDはどちらもexit101で、同じ成功として集計しない。全workspaceは92 suite／891成功、failed／ignored 0。

ローカル検査時のcommit IDと公開API経由のcommit IDは異なり得る。GREEN開始時のHEADは`910e7b6`で、実行したruntimeには直前のrustfmt差分があり、`a809ae7`へ保存した。実行中にruntimeのbytesは変えていない。元runnerのtree記録時点とHEAD記録時点の違いもverificationへ残した。測定・fuzz・fmt-checkは`a809ae7`と同tree。公開時はsource treeを照合する。[設計・保証範囲と残課題](../../../docs/internal/sqlite-acquire-budget-results.md)を参照。
