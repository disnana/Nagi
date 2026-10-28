# runtime依存の選択（0.2.2）

[English](en/runtime-profiles.md)

この機能は公開済みNagi 0.2.2で利用できます。0.2.1以前にはありません。`nagic build --help`に`--runtime-profile full|auto`があることを確認してください。プラグインの版とは別です。

最初の小さなプログラムでも通信・暗号ライブラリをビルドする負担を減らすため、依存runtimeの選択を明示的に試せます。

```sh
nagic run --project my-app --profile dev --runtime-profile auto --build-stats
```

既定の`full`は従来の全runtime APIを使います。`auto`は、完全検査後のIRから全生成関数・型・操作が対応範囲内と確認できた場合だけ`core`を選びます。現在の対象はHelloやPointに相当する同期処理、プリミティブ値、単純なデータ型、直接呼び出し、print、対応する演算・フィールド参照です。未使用関数も確認します。HTTPやDBアプリ全般を軽量化する機能としては扱わないでください。

coreも、データ型の生成に必要なSerdeとSQLiteの`FromRow`等を保持します。SQLセッション・HTTP・認証・TLS等はfullのAPIです。必要な場合は通常構成へ戻ります。非同期・未知の構文/型/操作も初版ではfullを選びます。

## 選択結果を確認する

`--build-stats`の`runtime`に要求・選択結果・理由を出します。次は形式の例で、時間の測定値ではありません。

```json
{"runtime":{"requested":"auto","selected":"core","reason":"eligible_core"},"checks":"full","cargo_always_invoked":true}
```

| reason | 意味 |
|---|---|
| `full_requested` | 既定または明示full |
| `eligible_core` | 対応するIRとruntimeを確認しcoreを選択 |
| `unsupported_ir` | 未対応の型・操作・構文、async等を含む |
| `incremental_requested` | 本体/関数単位CASはfullを使う |
| `native_input` / `rust_adapter` / `rust_dependencies` | 追加入力・adapter・Rust依存の境界でfullを使う |
| `runtime_unsupported` | core対応宣言・固定配置・共有ソースの同一性を確認できない |

`auto`と`--incremental`または`--incremental=units`を併用しても、初版のruntimeはfullです。既存CASの再利用は従来の条件で行います。coreを強制して未対応APIを削除するオプションはありません。

## 安全性とキャッシュ

High/Lowの完全な構文・型・所有権・Policy検査、Rust生成、Cargo起動は毎回行います。Rustコードの文字列やimport名だけで依存を削りません。Cargoのfeatures、manifest、lock、toolchain、profile等に応じたnative再判定はCargoが行います。Nagiのhashだけで実行ファイルを再利用しません。

失敗をfullで再試行して隠す機能ではありません。壊れたlockやCargo失敗は失敗として扱い、旧成功世代を維持します。既定releaseと最適化設定は変更しません。

full/coreの切替はCargo依存packageの変更として再判定されます。ソース、共有runtime、manifest、lock、features、toolchain、target、profile、build scriptやCargoが追跡する環境入力の変更もCargoの責任で再判定します。生成本体/unitsのNagi CASは今回変更せず、auto要求時もfullへ戻して既存の無効化条件を維持します。プロジェクト間で独自にnative成果物を復元する機能は追加していません。

## 比較する

計測スクリプトはPython 3.11以上を使います。先にリポジトリで`cargo fetch --locked`を行い、比較対象の依存を同じCargo cacheへ用意してください。測定中はofflineで実行します。

```sh
python scripts/benchmark_runtime_profiles.py --compiler path/to/nagic --work-dir build/runtime-bench --output build/runtime-bench.json --repeat 3 --prune-owned-targets
```

同じコンパイラ・入力パス・lockを使い、通常構成とautoを交互に測ります。Hello/Pointそれぞれでcold、変更なし、本体・公開形状の変更、runtime変更、lock破損からの拒否・回復を確認します。coldは新しいCargo targetであり、registry/OS cacheは温まっています。既定の測定条件はdev・debug情報0・2 jobsで、通常dev profileと同一と誤認しないでください。

計測のseed lockはrunnerが出力先へ配置します。通常のbuildは出力先の既存Cargo.lockを継承し、なければCargoが依存を解決します。初回のindex更新・download時間はこのoffline測定に含みません。

出力にはwall/CPU/最大子プロセスRSS、Cargo Fresh/Compiled、容量、中央値と範囲、native出力検査を記録します。RSSは同時実行プロセス全体の合計ではありません。起動時間は本番throughputの比較ではありません。`--prune-owned-targets`はこの計測で作った完了済みtargetだけを削除し、ログ・ソース・公開済み実行ファイルを残します。

[2026-10-24の実測](../benchmarks/results/runtime-profiles-2026-10-24/README.md)では、同じ環境・3回のPoint初回ビルド中央値はfull 60.68秒、auto 11.41秒でした。対象・条件を限定した結果で、未対応のアプリはfullを使います。

[既存の開発ビルド](development-builds.md)と[技術設計](internal/runtime-profile-selection.md)も参照してください。

[初回依存lockの明示選択](lock-seeds.md)も独立に指定できます。
