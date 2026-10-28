# Nagiで何を作るか

Nagi 0.2.2は、入力・出力・失敗を型で表し、HTTP・JSON・SQLiteやRustの資産を使う小さなバックエンドを作るための開発中の言語です。まず用途を選び、完成した例を動かしてからAPIリファレンスを読むと、断片的なコードだけで迷わずに済みます。

公開版の既定targetはRustです。0.3.0向けGo targetは実装・検証中で、公開版には含まれません。特にGoのHTTP・Actorは未対応です。[変更履歴](../CHANGELOG.md)の`Unreleased`は導入済み機能ではありません。

## 最初の小さなプログラムを動かす

0.2.2では、Pythonの`input(prompt)`に近い形で表示してから入力を読み、I/O失敗を`Result`として扱えます。

```nagi
def main() -> Result[unit, Error]:
    name = try read_line("名前 > ")
    return ok(print(name))
```

`nagic run hello.nagi --profile dev`でネイティブ実行します。`try`の成功値・失敗伝播・手動回復は[Resultとtryの教材](../test-nagi-code/result-try-tutorial/README.md)、入力を含む完全なCLIは[最初のCLIアプリ](first-app.md)にあります。

## 機能から探す

| 機能 | 作れるもの・用途 | Rust target 0.2.2 | Go target | 動く例 | 詳細 |
|---|---|---|---|---|---|
| 型・class・`Result`・`try`・ownership/view | 入力検証、集計、失敗を呼出元へ返すCLI | 公開済み | 未公開の部分実装 | [在庫集計CLI](../test-nagi-code/application-examples/stock-report/README.md) | [言語ガイド](language-guide.md)・[エラー処理](error-handling.md) |
| HTTP server・JSON・入力上限 | JSON API、明示したstatus/error、有限のrequest | 公開済み | 未対応 | [タスク管理API](task-api-tutorial.md)・[完全なsource](../test-nagi-code/web-demo/tasks.nagi) | [HTTP](http.md)・[JSON](json.md) |
| SQLite・型付きParameters・Pool/Tx | CRUD、nullable値、再起動後の保存 | 公開済み | 未対応 | [機器設定API](../test-nagi-code/application-examples/device-settings/README.md) | [SQLite](database.md)・[Pool/Tx](sqlite-pool.md) |
| async・scope・Task handle | 待機、子処理のjoin、型付き結果と取消 | 公開済み | 未公開の実験段階 | [Task結果](../test-nagi-code/library-examples/task-results/README.md) | [async](async.md)・[Task](task-handles.md) |
| Actor・Supervisor | 状態を持つworker、one-for-one再起動、停止 | 公開済み | 未対応 | [監視付きservice](../test-nagi-code/library-examples/supervised-service/README.md) | [Actor](actor.md)・[Supervisor](supervisor.md) |
| Rust crate adapter | Axumや既存crateを明示した境界から呼ぶ | 公開済み | Rust資産は対象外 | [Axum見積API](../test-nagi-code/application-examples/axum-service/README.md) | [Rust連携](modules-and-rust.md) |
| dev build・incremental・runtime/lock選択 | 編集時の待ち時間と依存解決を明示的に選ぶ | 公開済み（runtime/seedはopt-in） | 未公開 | [開発ビルド](development-builds.md) | [runtime選択](runtime-profiles.md)・[lock seed](lock-seeds.md) |

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
- PostgreSQL、WebSocket、HTTP/2、streaming、サーバー側TLSを標準APIだけで必要とする場合、Nagi 0.2.2では不足します。
- Rustのtrait、generic、async ecosystemを制限なく使いたい場合は、RustとAxumを直接使う方が適しています。
- 成熟した大規模運用実績、豊富なmiddleware、長期互換性が採用条件なら、Nagiの現在の開発段階を考慮してください。

同じタスク管理APIをNagi・FastAPI・Quartで実装した条件は[具体的な比較](task-api-comparison.md)にあります。速度順位ではなく、コード、依存、検証、配布、現在の制限を揃えて選ぶための資料です。
