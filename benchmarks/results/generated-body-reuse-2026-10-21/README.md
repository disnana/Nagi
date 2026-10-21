# 生成本体再利用のアプリbuild計測

公開0.2.0後の開発ソースで、通常のdevと`--profile dev --incremental`を比較した。runtime・依存版・Cargo profileを変更せず、完全なNagi検査とCargoを毎回実行する。[raw samples](raw.json)、[compiler/archive同一性](provenance.json)、[設計と制約](../../../docs/internal/generated-body-reuse.md)を参照。

## 条件

Linux x86_64、Rust/Cargo 1.99.1、OSが報告するCPU数5、cgroup CPU quota 4相当の共有host。サンプルは逐次実行し、その間に別buildを並走させていない。実runtimeを使い、同じsource pathに1,000個の小さな関数と、それぞれを呼ぶmainを作る。独立した空のtargetと同じseed Cargo.lockから開始する。registry/OS cacheは温まっており、networkはoffline。coldは依存済みtargetからのbuildを指さない。

比較元compilerは前回の4 OS検証済みPR #138のarchive（source `91b27e6`）とbyte単位で一致する。そのmain反映`1386411`とRust入力は同一。今回compilerのRust入力は`5a90083`。Cargoのprofile・rustflags・wrapper環境上書きがある環境をharnessは拒否する。両条件とも標準devのdebug/incremental設定をそのまま使い、前段で試したdebug情報削減案は採用していない。

## 結果

| 観測 | 通常dev | 本体再利用dev |
|---|---:|---:|
| 空targetからのbuild、1回 | 38.453秒 | 38.522秒 |
| 変更なし、statsなし、5回中央値 | 0.713秒 | 0.398秒 |
| 変更なし、statsあり、5回中央値 | 0.819秒 | 0.490秒 |
| 同条件のCargo処理、5回中央値 | 470.8ms | 147.1ms |
| コメントのみ変更、statsあり、1回 | 0.807秒 | 0.507秒 |
| 関数内部変更、statsあり、1回 | 0.830秒 | 0.862秒 |
| 公開署名変更、statsあり、1回 | 0.870秒 | 0.830秒 |
| warm最大child RSS、5回中央値 | 214.5MiB | 130.4MiB |
| 最後のtarget容量 | 1,347.2MiB | 1,264.5MiB |
| 14成功世代を含むout容量 | 252.6MiB | 254.5MiB |

このfixtureの変更なしbuildはstatsなしで約44%、statsありで約40%短くなった。coldはほぼ同じ。本体・署名の変更はlibrary全体を再コンパイルするため、常に速くなるとは主張しない。single sampleの数十ms差は一般化しない。

5回のwarmとコメント変更の計6回で、生成本体はsource hitかつCargo-freshだった。コンパイルされたartifactは起動binの1個だけ。本体変更と署名変更はsource miss、本体Cargo-fresh=false、bodyとshimの2 artifactになった。全sampleで実行出力を確認し、観測付き18世代のschema 1/2 receiptも照合した。関数内部変更は1,000関数のうち一つだけだが、現段階では本体crate全体を再コンパイルする。

RSSは`wait4`が返すchildのhigh-water markであり、同時に存在する全processの合計ではない。outには大きいdebug executableと各世代のsnapshotを残している。容量減少や厳密なcache上限を保証せず、source cacheも自動削除しない。native実行の性能差、他OSの性能、CI全体時間・費用、cross-language比較は測っていない。

## 再現

```sh
CARGO_NET_OFFLINE=true python3 scripts/benchmark_incremental_builds.py \
  --compiler target/release/nagic --baseline /path/to/previous/nagic \
  --work-dir local-only/incremental-bench --output local-only/incremental.json \
  --functions 1000 --repeat 5
```

独立した作業directoryへsource、log、cache、snapshotを作り、既存cacheを削除しない。比較元も`--profile dev --build-stats`を持つ開発compilerが必要。公開0.2.0にはこれらのflagがない。CI generatorは引き続き公開版に対応したplain buildを生成する。
