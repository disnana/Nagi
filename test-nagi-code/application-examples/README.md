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
| [バイト列API](byte-inspector/README.md) | 本文を借りて読む、u8の反復・index、nullableとJSON応答 |
| [Axum見積API](axum-service/README.md) | RustのHTTP層からNagiのasync業務処理を呼び、Resultを応答へ変換 |
| [認証・認可の型境界](auth-boundary/README.md) | 標準HTTP policy、dispatcher生成AuthScope、Nagi policyと消費型Grantを使うDB操作 |

手書きLowから始める場合は、[注文見積もりCLI](../low-examples/order-quote/README.md)を使ってください。Low同士のimport、型付きJSON、入力検証と整数の価格計算を試せます。

バイト列APIにはNagi 0.1.10以降が必要で、Axum見積APIは0.1.10で検証しています。認証例はこのbranchの標準HTTP request-bound auth APIを使います。その他の既存例はNagi 0.1.9でも動きます。新APIの公開配布での利用可否は公式Release記録を確認してください。サンプルはリポジトリ一式を取得し、そのルートで各フォルダの`nagi.toml`を指定して実行してください。Rust/CargoとOSごとのビルド環境は[セットアップ](../../docs/getting-started.md)で確認できます。

```sh
nagic run --project test-nagi-code/application-examples/stock-report
```

検証スクリプトは10プロジェクトを扱います。9つのHighアプリは元のソースと保存した生成Lowから別々にcheck・buildし、注文見積もりCLIは手書きLowから検証します。計19回の検証で同じ入力・ファイル・HTTP・actorの動作を照合します。認証・認可の例には、このブランチのコンパイラが必要です。Python 3.12以降を使い、リポジトリのルートで実行してください。

```sh
python scripts/verify_application_examples.py --compiler /path/to/nagic
python scripts/verify_application_examples.py --compiler /path/to/nagic --only device-settings
python scripts/verify_application_examples.py --compiler /path/to/nagic --only supervised-worker
python scripts/verify_application_examples.py --compiler /path/to/nagic --only order-quote
```

ビルドの警告も失敗として扱います。結果・ビルドログ・実行ログは`build/application-example-verification/`に残ります。DBとファイルの検証には一時的な保存先を使います。見積APIの検証はlocalhostでサーバーを起動・停止します。監督付きworkerでは意図的なpanicの診断がstderrに出ますが、回復と後片付けを検証して終了コード0で終わります。
