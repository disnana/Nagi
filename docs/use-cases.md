# Nagiで何を作るか

Nagi 0.2.1は、入力・出力・失敗を型で表し、HTTP・JSON・SQLiteやRustの資産を使う小さなバックエンドを試すための開発中の言語です。まず用途を選び、完成した例を動かしてからAPIリファレンスを読むと、断片的なコードだけで迷わずに済みます。

## 今すぐ試せる用途

| 作りたいもの | 最初に動かすもの | 確認できること |
|---|---|---|
| JSONを読むCLI | [在庫集計CLI](../test-nagi-code/application-examples/stock-report/README.md) | 型付きJSON、入力検証、独自Error、集計 |
| SQLiteを使うJSON API | [タスク管理APIの通し教材](task-api-tutorial.md) | HTTP route、JSON、検証、CRUD、再起動後の保存、テスト |
| 設定保存API | [機器設定API](../test-nagi-code/application-examples/device-settings/README.md) | NULL、bool、float、BLOB、SQLite Pool/Tx |
| RustのWeb基盤と組み合わせるAPI | [Axum見積API](../test-nagi-code/application-examples/axum-service/README.md) | AxumのHTTP層からNagiの型付きasync関数を呼ぶ境界 |
| 状態を持つworker | [予約worker](../test-nagi-code/application-examples/seat-reservations/README.md) | 業務Err、actor状態、Supervisorによる再起動 |
| Rust crateを使うアプリ | [ローカルRustライブラリ](../test-nagi-code/rust-library/README.md) | path依存、feature、手書きadapter |

## Nagiを候補にしやすい条件

- Python風の字下げ構文で書きながら、関数の型、`Result`、move/viewを明示したい。
- 日常の編集では`nagic check`を先に使い、配布時はネイティブ実行ファイルを作りたい。
- HTTP・JSON・SQLiteで足りる小さなサービスか、足りない部分を小さなRust adapterに分けられる。
- 型・所有権・capabilityの境界をコードレビューで追えることを重視する。

## 別の選択肢を先に検討する条件

- Pythonの既存packageやデータ処理資産を直接使いたい場合は、FastAPIやQuartの方が自然です。
- PostgreSQL、WebSocket、HTTP/2、streaming、サーバー側TLSを標準APIだけで必要とする場合、Nagi 0.2.1では不足します。
- Rustのtrait、generic、async ecosystemを制限なく使いたい場合は、RustとAxumを直接使う方が適しています。
- 成熟した大規模運用実績、豊富なmiddleware、長期互換性が採用条件なら、Nagiの現在の開発段階を考慮してください。

同じタスク管理APIをNagi・FastAPI・Quartで実装した条件は[具体的な比較](task-api-comparison.md)にあります。速度順位ではなく、コード、依存、検証、配布、現在の制限を揃えて選ぶための資料です。
