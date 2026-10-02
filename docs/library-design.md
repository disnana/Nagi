# ライブラリとRust連携の設計案

このページは提案です。moduleの名前空間、詳細なCargo依存設定、不透明なresource型、汎用DB APIはまだ実装されていません。以下の構文案は、現在の実行例とは区別してください。

[目次](README.md) · 現在使える機能: [importとRust連携](modules-and-rust.md)、[nagi.toml](projects.md)

Nagiで書いた型・検証・計算を複数のアプリで共有し、通信や保存には既存のRustライブラリを組み合わせる構成を目指します。CLI、HTTP、バッチ処理ごとに業務規則を書き直す必要を減らします。

## 現在の対応と不足している部分

| 部分 | 現在 | 提案する追加 |
| --- | --- | --- |
| import | 相対ファイルを同じ名前空間に読み込む | module、`from`、`as`、別moduleの同名定義 |
| Rust連携 | sync／asyncのexternと型付きの引数・戻り値 | 接続やclientを表す不透明な型 |
| Rustファイル | `rust.file` で1つのnative moduleを指定 | 既存形式を保ち、共有crateも指定できる依存設定 |
| Cargo依存 | crate名とversion文字列 | path・features・default-features・package |
| ランタイム | HTTP、JSON、SQLite等を常に依存に含む | 必要な機能に合わせた依存とコード生成 |
| DB | SQLiteと固定したbind引数 | 型付きの任意個の引数、行読み取り、transaction |

現在のexternはRustのAPIを自動でimportする機能ではありません。Rust固有の型は、既知の数値・str・class・List・Result等へ変換します。Nagiの `check` は宣言と呼び出しを検査し、Rust実装との一致は `build` で確認します。externからviewを返すことは未対応です。

## 共有する処理とアダプター

共有するNagiファイルにデータ型・検証・計算を置き、入口で入出力を組み合わせます。Rustのアダプターは外部ライブラリの型をNagiのデータ型へ変換します。独立したRust crateは自身の型を使い、生成アプリのclassへの変換はアダプターに置きます。

現在の6つの例は、この分け方を既存APIで試すものです。

| プロジェクト | 再利用と連携の例 |
| --- | --- |
| [foundation-cli](../test-nagi-code/library-examples/foundation-cli/README.md) | 共有の `foundation.nagi` とRustの `pricing.rs` をCLIから使う |
| [foundation-report](../test-nagi-code/library-examples/foundation-report/README.md) | 同じ検証・計算をJSONレポートに使う |
| [rust-json](../test-nagi-code/library-examples/rust-json/README.md) | serde_jsonの結果をNagiのclassへ変換する |
| [rust-async](../test-nagi-code/library-examples/rust-async/README.md) | Nagiの実行環境でRustのTokioタイマーをawaitする |
| [custom-http](../test-nagi-code/library-examples/custom-http/README.md) | RustのAxum／TokioサーバーへNagiの同期callbackを渡す |
| [low-kernel](../test-nagi-code/library-examples/low-kernel/README.md) | アプリの処理から手書きLowの計算を呼ぶ |

`rust.file` が1つでも、Rustの `mod` や `#[path]` で実装を分割できます。custom-httpは組み込みのserveやDbを使いません。HTTPの制限・停止処理はそのRust側で管理し、組み込みHTTPの設定が自動適用されるとは扱いません。

## moduleと名前解決

次は未実装の構文案です。

```nagi
import json
import sqlite as storage
from json import decode as decode_json
import "domain/orders.nagi" as orders
from "domain/orders.nagi" import Order as SavedOrder
```

`orders.Order` と `SavedOrder` は同じ定義を指し、別ファイルの同名classは別の型です。moduleと定義にIDを持たせ、型検査、High→Low、Rust出力、エディターが同じ解決結果を使います。別名を文字列置換して実装しません。

標準moduleは同梱の定義、引用符付きimportは相対ファイルを指します。importだけでDB接続や通信は始めません。初版はそのファイルに定義した関数・classを公開し、importした名前を自動で再公開しません。既存の平らなimportと組み込み関数は互換入口として保ちます。

## Cargoの依存設定

次のtable形式は提案です。現在は `serde_json = "1.0"` のようなversion文字列だけを受け付けます。

```toml
[rust]
file = "adapters/native.rs"

[rust.dependencies]
serde_json = "1.0"
foundation = { package = "my-foundation", path = "../my-foundation" }
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }
```

文字列形式を保ち、table形式にversion・path・features・default-features・packageを追加します。pathはnagi.toml基準で解決し、生成先を変えても同じcrateを参照します。依存解決はCargoに任せます。CLIのversion指定とtable設定の上書き規則も明示します。

Cargoのfeaturesは依存経路ごとに合成されます。直接依存でdefault-featuresを無効にしても、別の経路が有効にした機能までは外れません。実際の依存ツリーで確認します。versionは許容範囲、Cargo.lockは解決した版なので、生成lockの保管と `--locked` ビルドを整理します。現在のnagic buildは `--locked` を付けません。

## ランタイムを機能ごとに選ぶ

core・async・json・http・sqlite・postgresに分ける案です。出力する関数・型・組み込みの解決結果から必要な機能を集めます。全関数を出力する段階では、入口から直接呼ぶものだけを見て依存を外しません。

classへのSerde／行読み取りの生成、公開型、エラー変換も切り替える必要があります。Cargoのoptional化だけでは終わりません。Rust連携には互換設定を保ち、新しいアダプターは必要なランタイム機能を明示できる形にします。依存crateのfeatureは生成アプリの同名featureへ自動では伝わりません。

組み込みHTTPには `serve(port)` を追加し、既存の `serve(db, port)` を保つ案です。登録routeにDb引数があれば引数1個のserveをエラーにします。これはcustom-httpのような現在の独自サーバーとは別の変更です。

## 不透明な型と非同期処理

clientや接続プールをNagiへ公開するには、型名とRust型の対応、操作、所有権の登録が必要です。現在のclassはデータ用であり、このresource型の代わりにはしません。

| 契約 | 必要な扱い |
| --- | --- |
| 所有・共有 | 原則move。Copyにせず、共有・cloneを許す型だけ明示する |
| 借用 | 読み取りと排他的な操作を分け、await中もownerを保つ |
| 型と変換 | 別providerの同名型を区別し、JSON／DB用deriveを自動追加しない |
| スレッド | Sendはスレッド間の移動、Syncは参照の共有を許す。両方を無条件に要求・付与しない |
| 終了 | close・commit・rollbackとdrop時の残り処理を定義する |

通常のDropではawaitできません。必要な非同期終了処理は明示的に用意します。futureのdropで呼び出し側をキャンセルしても、別workerや既に送ったDB書き込みが取り消される保証はありません。終了・再試行・二重書き込みへの対応は操作ごとに決めます。

## 汎用DB APIの前提

現在のdb_insertはstrとi32、db_updateはi64・str・i32を取ります。互換用に残し、任意のテーブルや条件に対応したAPIとしては説明しません。

| 部分 | 先に定義する契約 |
| --- | --- |
| 引数 | 任意個の型付き値、NULLの型、str／bytes／viewの所有権 |
| 操作 | execute／one／all。対象なしはOption、bindなしも同じ規則で扱う |
| 行 | 元のfield名で列を対応付け、列順・NULL・整数範囲・型違いを扱う |
| transaction | 1接続を保持し、commit／rollbackで消費する。終了後の使用と同時操作を禁止する |
| エラー | 接続・待機・SQL・decode・対象なしを区別する |

型付きの可変長引数を第1候補とします。SQLとschemaの一致はbackendでも検査し、動的SQLまでコンパイル時に保証しません。transactionの排他的借用には現在のread-only viewとは異なる扱いが必要です。

SQLiteの `?1`、PostgreSQLの `$1`、i64に対応するBIGINTやidentity等の違いは保存層で扱います。SQLは自動翻訳せず、poolの別々の呼び出しでBEGIN／COMMITを送るtransactionも作りません。一般利用向けPostgreSQL対応は、引数・行・transactionの契約と実DB試験が揃ってから案内します。

## 実装する順序

1. 現在の例で共有処理とadapterの境界を確認し、Nagiの検査とRust buildを両方通す。
2. 詳細な依存設定、ランタイム機能の選択、derive生成、lockの保管を整える。
3. module、from/as、High→Low、エディターを同じ名前解決へ揃える。
4. 組み込みHTTP起動をDBから分け、DBなしではworkerもSQLite依存も不要にする。
5. resourceの所有・借用・キャンセルと汎用DB契約をSQLiteで検証する。
6. PostgreSQLで同じ契約、TLS・pool・timeout・停止を実DB検証し、保存先を変えても同じ業務処理を使う例を作る。

旧import・組み込み・Rust連携を保ち、採用した部分から移行できるようにします。各段階で既存例、High／Low、エディター、対象OSを確認します。依存削減はCargoのfeatures、ビルド時間、出力容量で測ります。

参考: [Cargo依存指定](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html)、[featuresと合成](https://doc.rust-lang.org/cargo/reference/features.html)、[Cargo.lock](https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html)、[Rust module](https://doc.rust-lang.org/book/ch07-02-defining-modules-to-control-scope-and-privacy.html)、[Send](https://doc.rust-lang.org/std/marker/trait.Send.html)／[Sync](https://doc.rust-lang.org/std/marker/trait.Sync.html)、[Future](https://doc.rust-lang.org/std/future/trait.Future.html)、[Drop](https://doc.rust-lang.org/std/ops/trait.Drop.html)、[Tokioのキャンセル](https://tokio.rs/tokio/tutorial/select)。
