# アプリケーションのサンプル

[English](README.en.md)

| プロジェクト | 内容 |
| --- | --- |
| [在庫集計CLI](stock-report/README.md) | 日本語を含むJSON入力、検証、独自エラー、集計 |
| [SQLite設定API](device-settings/README.md) | NULL・bool・float・BLOB、HTTP応答、再起動後の保存 |
| [予約worker](seat-reservations/README.md) | 業務エラー、Supervisorによる再起動、別actorの状態保持 |
| [JSON設定ファイル](file-json/README.md) | 型付きJSON、Rustのファイル操作、既存ファイルの保護 |
| [見積API](quote-api/README.md) | DBなしのHTTP、共有設定、独自エラーとroute別の応答変換 |
| [監督付きworker](supervised-worker/README.md) | actorの再起動、taskのpanic回復、停止と後片付け |

手書きLowから始める場合は、[注文見積もりCLI](../low-examples/order-quote/README.md)を使ってください。Low同士のimport、型付きJSON、入力検証と整数の価格計算を試せます。

各フォルダの`nagi.toml`を指定して実行できます。最新ソースからビルドしたNagiを使ってください。Rust/CargoとOSごとのビルド環境は[セットアップ](../../docs/getting-started.md)で確認できます。

```sh
nagic run --project test-nagi-code/application-examples/stock-report
```

リポジトリのルートで次を実行すると、Highのアプリは元のソースと保存した生成Lowから別々にcheck・buildし、同じ入力・ファイル・HTTP・actorの検証を行います。注文見積もりCLIは手書きLowを直接検証します。Python 3.12以降が必要です。

```sh
python scripts/verify_application_examples.py --compiler /path/to/nagic
python scripts/verify_application_examples.py --compiler /path/to/nagic --only device-settings
python scripts/verify_application_examples.py --compiler /path/to/nagic --only supervised-worker
python scripts/verify_application_examples.py --compiler /path/to/nagic --only order-quote
```

結果・ビルドログ・実行ログは`build/application-example-verification/`に残ります。DBとファイルの検証には一時的な保存先を使います。見積APIの検証はlocalhostでサーバーを起動・停止します。監督付きworkerでは意図的なpanicの診断がstderrに出ますが、回復と後片付けを検証して終了コード0で終わります。
