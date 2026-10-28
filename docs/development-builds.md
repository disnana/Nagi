# 開発ビルドと変更の観測

[English](en/development-builds.md)

このページの `--profile` / `--build-stats` と差分ビルドは公開済み0.2.1の機能です。公開0.2.0には含まれません。`nagic --version`を確認し、[インストール手順](getting-started.md)で更新できます。既定の `build` / `run` は従来どおりreleaseです。

```sh
nagic check --project my-app
nagic run --project my-app --profile dev
nagic build --project my-app --profile release --build-stats
```

`check` はRustをコンパイルせず、読み込んだHigh・Lowの構文、型、所有権、標準Policyの型境界を検査します。SQL schemaの検査は従来どおり明示指定です。アプリの業務認可、Rust adapterの実装、最終Send/Sync、リンクや配備先の状態を証明するコマンドではありません。

`dev` はCargoの開発profile、`release` は既存の最適化profileを使います。依存runtimeの開発用コンパイルを再利用でき、最適化待ちを減らせます。整数の加減乗算などのoverflowは通常devでpanic、releaseでwrapします。既存の言語契約です。Cargo設定や環境でprofileを上書きした場合はその設定にも従います。本番性能の測定と配布はreleaseで行ってください。

公開0.2.2では、対応する小さなプログラムの依存を減らす[明示opt-inのruntime選択](runtime-profiles.md)を利用できます。0.2.1以前には未対応で、既定fullと完全検査は維持します。

## 生成本体の差分再利用

```sh
nagic run --project my-app --profile dev --incremental --build-stats
```

生成Rust本体と依存manifestのSHA-256を使い、アプリごとの変更しないCargoライブラリを作ります。生成Rustが同一なら本体のコンパイル済み成果物をCargoが再利用し、世代固有の小さな起動コードだけを再コンパイルします。公開済みの実行ファイルは引き続き世代ごとにコピーし、上書きしません。

完全検査、コード生成、Cargo起動は毎回行います。本体の変更で新しい内容キーを作り、ランタイムの変更、Rust設定、toolchain等のnative成果物の有効性はCargoが判定します。意味hashや前回の解析JSONだけで実行ファイルを再利用しません。関数・型・module単位のnative差分コンパイルではなく、生成本体を一つのRust crateとして再利用する段階です。

初版は`--profile dev`での明示指定です。Rust adapter、追加Rust依存、IDE管理runの保持設定がある場合は既存の単一bin構成へ戻り、標準エラーとstatsの`generated_body.reason`に理由を出します。既定のbuildとreleaseは変わりません。

statsの`generated_body`は`mode`、生成ソースの`source_cache`（`hit` / `miss`）、`sha256`、本体成果物の`cargo_fresh`を示します。ソースcacheのhitだけではnativeの再利用成功を意味しません。保存内容を今回の生成内容とbyte単位で照合し、不一致・link・不正なファイル種類・想定外の追加ファイルやディレクトリは失敗にします。勝手に修復せず、旧latestと旧実行ファイルを保持します。

本体は出力内の`.nagi/apps/<app>/code/<sha256>/`に保持し、自動削除しません。内容キーのcheckout間の移植可能性や共有cacheの認証は保証しません。協調しない別processによるディレクトリの同時差し替えやCargo cacheの改変は保証範囲外です。

## 関数の依存関係によるRust分割

```sh
nagic run --project my-app --profile dev --incremental=units --build-stats
```

型検査済みの生成planから直接callの依存関係を調べ、安全に独立できる関数群を最大8個のCargo libraryへ分けます。相互再帰する関数は同じ単位に置きます。変更した単位とその呼び出し元は再コンパイルされ、無関係な単位のRustソースとCargo成果物を再利用できます。関数内部の編集でも、その関数だけの機械語を差し替える仕組みではありません。小さな関数は同じ単位にまとめられます。

初版の切り出し対象は、引数・戻り値・内部の値が`i32`、`i64`、`bool`、`unit`で、直接callと対応する演算・分岐・loopからなる同期関数です。async、ユーザー型、標準API、関数値などを扱う関数と、それらを呼ぶ関数は既存の本体へ残します。本体は全単位へ依存し、未知のcallback関係があっても保守的に再コンパイルします。HTTPや非同期アプリに独立した計算関数がある場合も利用できます。対象がなければ通常の本体再利用へ戻り、`generated_body.partition_reason`に理由を出します。adapter・追加Rust依存・IDE管理runの単一binへのfallbackは同じです。

statsの`generated_body.mode`は`cargo_units`になり、`units`に各単位の関数、内容キー、`source_cache`、`cargo_fresh`を出します。完全検査とRust生成・Cargoは引き続き毎回行います。型や宣言の並べ替えだけで不要なRust差分が出ないよう、切り出す関数は安定したシンボル順で生成します。関数の追加やcall graphの変更で単位の構成が変わる場合は、より広い範囲が再コンパイルされます。

receipt schema 3は本体から参照する全単位を検査します。対応する全source cacheが必要です。検証器は固定されたmanifest形、最大9ノードの依存閉包、各内容キーと実体、runtime・root manifestの一致を確認します。schema 1/2の対応も維持します。共有native cacheの認証やcheckout間の移植性を追加するものではありません。

## 時間と再ビルド理由を読む

`--build-stats` は成功時に `nagi-build-stats: {JSON}` を標準エラーへ1行出します。`run` のアプリ標準出力は変えません。時間の単位はmsです。

- `before_cargo_ms`: 入力読込み、完全検査、生成、出力lock待ち、観測記録を含むCargo起動前の時間。
- `cargo_ms`: Cargo起動から終了まで。`total_ms` は公開・receipt検査用hash作成も含み、アプリ実行時間は含みません。
- `cargo_fresh_artifacts` / `cargo_compiled_artifacts`: Cargoの `compiler-artifact.fresh` の件数。依存crateとアプリを含むartifactの観測であり、関数単位のヒット率やsccacheのヒット率ではありません。
- `observed_change`: 前回の観測との比較。初回・記録欠損、profile、宣言形状、関数本体、生成Rust、変更なしを区別します。外部依存を含むCargoの再ビルド理由全体ではありません。

成功世代に `build-analysis.json` と `build-receipt.json` を保存します。前者は型付きProgramの宣言形状、関数ごとの正規Low、生成RustのSHA-256、module／直接call／型関係のgraphを保持します。ソース行番号を関数内容のhashに混ぜません。宣言形状は全読込定義の署名で、言語のexport API互換性判定そのものではありません。関数値やcallbackなど、既存graphで追跡できない関係はwarningsに残ります。

後者は公開されたLow・Rust・manifest・lock・ソース／位置snapshot・解析記録・実行ファイルのSHA-256と長さ、およびcompiler実体のSHA-256を保存します。記録のpathは成功実行ファイルの隣です。内容を再検査できます。

```sh
python3 scripts/verify_build_receipt.py path/to/generations/g-.../build-receipt.json --compiler path/to/nagic
```

0.2.1配布物には同じ検証器を`tools/verify_build_receipt.py`として同梱します。Python 3.10以上で実行でき、ソースcheckoutは不要です。検証対象は通常ファイルに限定し、成果物のsymlink・Windows reparse point・FIFO等を拒否します。compiler引数にはinstallerが作る実行ファイルへのlinkも使えます。同時に悪意ある別processがdirectoryを差し替える環境の安全性は保証しません。

全検査・コード生成・Cargoを毎回実行し、新しい成功世代へ公開します。記録が壊れても検査は省略しません。receipt作成に失敗したbuildは旧latestを保持し、新しいアプリを実行しません。

本体再利用時のreceipt schema 2は、起動コードと本体manifestも記録します。検証器はsnapshotから内容キーを再計算し、所定のcache内の本体と照合し、生成manifestの依存先も確認します。通常buildのschema 1も引き続き検証できます。schema 2の検証には対応する本体cacheも必要です。

SHA-256の照合は破損を検出しますが、記録ごと改変できる攻撃者に対する署名・認証ではありません。Rust module/include、外部path crate、build script、環境、toolchain等の完全な入力閉包はNagiでは記録しません。receiptやNagiソースのhashだけで、CIから取得した実行ファイルの実行を許可しないでください。ソースsnapshotにはソース内の秘密も含まれ得るため、生成ディレクトリ全体を共有cacheへ保存しないでください。

## ローカルとCI

CI generatorはこれらのflagを自動では有効にしません。0.2.1以降のcompilerをCIに固定して、必要な開発用jobで明示指定してください。マージ前の完全検査と本番向けrelease buildは引き続き必要です。

既存の `NAGI_NATIVE_TARGET_DIR` で依存cacheを選べます。同じcacheでもアプリIDと成功世代は分離し、devの `debug/` とreleaseの `release/` を使い分けます。通常の成功世代は自動削除しないため、容量を監視してください。IDE内部の保持設定を一般CLIのGCとして使わないでください。

公開0.2.0で使える[プロジェクトCI生成](user-ci.md)、[実装計画と制約](internal/smart-build-plan.md)、[計測結果](../benchmarks/results/smart-build-2026-10-20/README.md)も参照してください。

moduleのIDにはcanonical pathが含まれます。この初版の意味hashを異なるcheckout間の移植可能なcache keyとは扱わないでください。

compilerのdigestは公開時に`current_exe()`のpathから読み直したファイルです。ビルド中にcompilerファイルを置換した環境では、実行中imageの同一性を証明しません。

`receipt` は表示用のpath文字列で、`receipt_os` は既存snapshotと同じOS単位（Unix bytes／Windows UTF-16）です。

[関数分割の実測](../benchmarks/results/function-unit-reuse-2026-10-21/README.md)
