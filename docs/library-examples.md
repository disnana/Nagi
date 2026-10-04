# サンプルプロジェクト

CLI、自作ライブラリ、Rust連携、HTTP、Supervisor、Lowの差し替えを試せます。各プロジェクトに`nagi.toml`と起動手順があります。標準HTTPと`std.actor`の例にはNagi 0.1.8以降が必要です。

## ライブラリと基盤を作る例

| プロジェクト | 試せること |
| --- | --- |
| [料金計算CLI](../test-nagi-code/library-examples/foundation-cli/README.md) | 入力を検証し、共通の計算処理を呼ぶ。Nagi実装とRust実装を切り替える |
| [JSONレポート](../test-nagi-code/library-examples/foundation-report/README.md) | CLIと同じライブラリを使う。一部の入力が不正でも、成功した行を集計する |
| [RustでJSONを読む](../test-nagi-code/library-examples/rust-json/README.md) | serde_jsonを使い、NagiのclassとResultへ変換する |
| [Rustの非同期処理](../test-nagi-code/library-examples/rust-async/README.md) | TokioのtimerをNagiからawaitする |
| [自作HTTP基盤](../test-nagi-code/library-examples/custom-http/README.md) | Axum/TokioへNagiの同期関数を渡し、DBなしでHTTP応答を作る |
| [標準HTTPと認証](../test-nagi-code/library-examples/http-auth/README.md) | Nagiだけでヘッダー・401・route別エラー・型付き共有状態を扱う |
| [SupervisorとHTTP](../test-nagi-code/library-examples/supervised-service/README.md) | actorが状態を順番に更新する。業務エラーと停止をHTTP応答へ変換する |
| [Low計算カーネル](../test-nagi-code/library-examples/low-kernel/README.md) | Highの呼び出しを変えずに、Lowの実装へ置き換える |
| [moduleと別名](../test-nagi-code/library-examples/module-imports/README.md) | 同名classを区別し、module名とfromの別名で同じ型を使う |
| [独自エラーのCLI](../test-nagi-code/library-examples/typed-errors/README.md) | enumで失敗を分け、元の原因を保持しながら表示文を選ぶ |

共通コードの置き方は[自作ライブラリとRustの資産](libraries.md)を参照してください。料金計算の2つのアプリは`shared/`も使うため、[サンプルのディレクトリ一式](../test-nagi-code/library-examples/)を取得してください。

## 動かす

Rust/Cargo、Nagi、お使いのOSでアプリをビルドできる環境が必要です。[準備と最初の実行](getting-started.md)で確認できます。

リポジトリのルートから、プロジェクトを指定して実行します。

```sh
nagic check --project test-nagi-code/library-examples/rust-json
nagic run --project test-nagi-code/library-examples/rust-json
```

プロジェクトのディレクトリへ移動した場合は、`nagic check`・`nagic run`だけで同じ設定を使います。料金計算CLIは入力待ちになり、HTTP基盤はCtrl+Cまで動きます。

ソースを1ファイルだけ指定すると、隣の`nagi.toml`を使わない形になります。Rust・Low・依存の設定を含めて動かすには、上の`--project`か、そのディレクトリでの引数なしコマンドを使ってください。

## アプリ全体を読む

| プロジェクト | 内容 |
| --- | --- |
| [タスク管理](../test-nagi-code/web-demo/README.md) | ブラウザー画面、JSON API、SQLiteへの保存 |
| [Result API](../test-nagi-code/result-api/README.md) | 入力不正、対象なし、DB失敗、代替データへの回復 |
| [在庫管理](../test-nagi-code/README.md#在庫管理api) | 型付きJSON入力、DBのCRUD、集計 |
| [在庫JSONの集計](../test-nagi-code/application-examples/stock-report/README.md) | stdinから型付きJSONを読み、独自エラーで入力を検証する |
| [機器設定API](../test-nagi-code/application-examples/device-settings/README.md) | SQLiteのNULL・bool・浮動小数点・BLOBを読み、再起動後も設定を保持する |
| [予約worker](../test-nagi-code/application-examples/seat-reservations/README.md) | 重複予約・残席不足を扱い、一方のactorだけを再起動する |
| [Axumの見積API](../test-nagi-code/application-examples/axum-service/README.md) | Rust/AxumでHTTPを受け、Nagiの名前付きasync関数から計算結果をResultで受け取る |
| [Rust連携](../test-nagi-code/rust-bridge/) | CRC-32、serde_json、非同期Rust関数の呼び出し |
| [フラクタル](../test-nagi-code/README.md#exe単体で見られるフラクタル) | コンソール表示とexe配布 |

各READMEで入力、出力、制約を確認できます。Rust連携の例が扱うのは記載したAPIと型です。

## 開発時にまとめて確認する

Python 3.12以降で、各プロジェクトのcheck・build・実行結果とHTTP応答を検証できます。リポジトリのサンプルと同じソースから作った`nagic`を指定してください。

```sh
python scripts/verify_library_examples.py --compiler /path/to/nagic
python scripts/verify_application_examples.py --compiler /path/to/nagic
```

料金計算の両実装の一致、moduleと型の別名、独自エラー、JSONの不正入力、非同期処理のエラー、HighとLowの結果の一致、HTTPの400・404・body上限・停止を確認します。Supervisorの例では更新と業務エラー後の状態保持、204の停止応答、停止後の503も確認します。ビルドは1つずつ行い、依存のキャッシュを共有します。

UnixではSIGINTによる正常終了、Windowsではプロセスを終了してlistenerが閉じることを確認します。WindowsのCtrl+Cによる終了は、HTTPサンプルの手順で手動確認してください。

`verify_application_examples.py`は[一覧のアプリ](../test-nagi-code/application-examples/README.md)を検証します。Highアプリは元のソースと保存した生成Lowから別々にcheck・buildし、手書きLowの注文見積もりCLIは直接検証します。入力の境界値、ファイルの保護、SQLiteの保存、HTTPのヘッダーとエラー、Supervisorの再起動・準備完了・停止を確認します。ビルドの警告も失敗として扱い、結果とログを`build/application-example-verification/`に残します。CIでは`build_examples.py`から実行します。
