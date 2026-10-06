# 一接続SQLite adapterの概測

2026-10-06、Linux x86_64、Rustのdebug profile、既存warm target。私有のdirect Driverとgeneric deadpool Manager adapterを同じnative core・hooks・memory DB・SQL・row decode・rollbackで比較した。

各backendで8回warmup後、32 batch×128 Tx、計4096 sampleずつ。batchごとにbackendの先行順を交互にした。測定区間はbeginから`SELECT 1 AS n`、row decode、rollback返信までで、openは含めない。最後に両方のnative closeとjoinをassertする。

| backend | p50 ns | p95 ns |
|---|---:|---:|
| direct | 106370 | 141402 |
| deadpool | 114402 | 152410 |

[summary.json](summary.json)にsource commit／tree、toolchain、算出法とhash、[samples.csv](samples.csv)に8192 sampleを保存した。p50／p95は昇順sampleの`(n-1)*percentile/100`を整数切捨てしたindex。CSVから原測定JSONと同じ値になることを確認した。

VM上の一条件の概測で、production性能、因果的overhead上限、統計的な差の証明ではない。release／競合／多接続、allocation count、Future size、memory、binary、compile timeは未測定。以前のNagi Future +32 byteの原因もこの比較では調べていない。

## 再実行

以下は測定artifactの出力先を指定する。既存targetを使うなら通常のCargo環境変数で設定する。公開APIのtimeoutやpolicyを変える変数ではない。

```sh
NAGI_SQLITE_MEASURE_OUTPUT=/tmp/sqlite-adapter-measurement.json \
  cargo test --locked -p nagi-runtime \
  sqlite_prototype::comparison::direct_and_deadpool_warm_native_session_comparison \
  -- --exact --nocapture
```

## 検証artifact

[verification.json](verification.json)は最終sourceの各ログのhashとRust test結果の計数。[audit](audit/)には重要なREDと最終43件、clippy／fmt／fuzz／単独測定のログを保存した。空のfmtログは出力なしで成功したコマンドの記録で、ログが空なだけで成功を推定したわけではない。

| ログ | 意味 |
|---|---|
| 05 | joinがterminal failure公開に先行した実際の契約違反 |
| 17／18 | create取消・take/drop後にnative workerが2となった実際の契約違反 |
| 23 | detach用seam未実装のE0560。契約違反の再現と区別 |
| 24 | stock detachのpermit先行返却でnative workerが2となった実際の契約違反 |
| 25 | 最終native22＋adapter20＋比較1が成功 |
| 28／29／30 | 最終clippy／fmt／既存fuzz smoke |
| 31 | 最後の独立測定。並列契約試験中の測定値とは分ける |

途中の19番ログは非Send MutexGuardによるcompile失敗で、名前にgreenがあっても成功の根拠に含めなかった。全ログ、途中RED source、final source、原JSONは別途ローカルartifact `nagi-phase4-adapter-evidence.tar.gz`へ保全した。archive SHA-256は`73c001623a11f9a647c614036b50a4b194da537b1b666e7c42d6162ea2ace01d`。このarchiveをrelease assetとして公開したという意味ではない。

[実装と限界](../../../docs/internal/sqlite-adapter-results.md)に、sourceの分担、再現した共通原因、未配線のpublic API／多接続／取得期限を記録した。性能値をtestの合否thresholdにはしない。
