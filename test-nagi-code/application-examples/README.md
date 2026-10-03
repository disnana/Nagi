# アプリケーションのサンプル

[English](README.en.md)

| プロジェクト | 内容 |
| --- | --- |
| [在庫集計CLI](stock-report/README.md) | 日本語を含むJSON入力、検証、独自エラー、集計 |
| [SQLite設定API](device-settings/README.md) | NULL・bool・float・BLOB、HTTP応答、再起動後の保存 |
| [予約worker](seat-reservations/README.md) | 業務エラー、Supervisorによる再起動、別actorの状態保持 |

各フォルダの`nagi.toml`を指定して実行できます。設定APIには最新ソースからビルドしたNagiが必要です。Rust/CargoとOSごとのビルド環境は[セットアップ](../../docs/getting-started.md)で確認してください。

```sh
nagic run --project test-nagi-code/application-examples/stock-report
```

リポジトリのルートで次を実行すると、3つのアプリをHighと生成Lowから別々にcheck・buildし、それぞれ同じ入力・HTTP・actorの検証を行います。Python 3.12以降が必要です。

```sh
python scripts/verify_application_examples.py --compiler /path/to/nagic
python scripts/verify_application_examples.py --compiler /path/to/nagic --only device-settings
```

結果・ビルドログ・実行ログは`build/application-example-verification/`に残ります。SQLiteの検証には一時的なDBを使います。既存のアプリのデータは使いません。
