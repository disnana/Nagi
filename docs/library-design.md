# ライブラリとRust連携の設計案

よく使う処理はNagiの型付きAPIで提供し、既存のRustライブラリを内部で使います。高度な処理や独自の基盤はRust連携で扱う方針です。Rustを知らずにアプリを書く人と、Rustの資産を組み合わせる人の両方を対象にします。

このページには未実装の案を含みます。現在の使い方は[importとRust連携](modules-and-rust.md)・[nagi.toml](projects.md)、動く例は[サンプル一覧](library-examples.md)を参照してください。

## 現在の対応と不足している部分

| 部分 | 現在使えるもの | 未実装・未確定 |
|---|---|---|
| import | 相対ファイル、`as`、複数名の`from`、登録済み標準module | package管理、公開範囲の指定 |
| 標準API | `std.http.server`、`std.actor` | JSON・DB等のmodule分離 |
| 標準resource | App・Request・Response・Actor・Supervisor等 | 利用者が定義するresourceの登録 |
| Rust連携 | sync／asyncのextern、型付きの引数・戻り値、手書きadapter | 任意のRust型・traitの直接利用、安定した外部ABI |
| Cargo依存 | version・path・features・default-features・package | ランタイム依存を使用機能だけに絞る生成 |
| DB | SQLite、固定形のbind、classへの行変換 | PostgreSQL、汎用引数、pool・transaction API |

`check`はexternの宣言とNagi側の呼び出しを検査します。Rustの実装やcrateとの一致は`build`で確認します。externからviewを返す機能はありません。

## 共有する処理とアダプター

型・検証・計算はNagiのmoduleへ置き、CLIやHTTPなどの入口から使います。Rustのadapterは、外部ライブラリの型をNagiの対応する型へ変換します。独立したRust crateにはそのcrateの型を使い、生成アプリのclassへの変換をadapterに置きます。

現在の例では、[HTTP認証](../test-nagi-code/library-examples/http-auth/README.md)は標準APIだけでヘッダーや業務エラーを扱い、[custom-http](../test-nagi-code/library-examples/custom-http/README.md)はRustのAxumサーバーから同期Nagi callbackを呼びます。後者の容量・期限・停止はRust側の責任です。標準HTTPの設定が自動適用されるとは扱いません。

## moduleと名前解決

同じmoduleを別名で読み込んでも同じ定義を参照し、別moduleの同名classは別の型です。IDは型検査・High/Low・Rust生成・エディターで使います。importした名前の自動再公開はありません。

`std.http.server`・`std.actor`の型と操作はコンパイラに登録されています。標準module名がローカルの同名ファイルへ解決されることはありません。通常のライブラリを追加するたびに言語のキーワードを増やす方針にはしません。

## Cargoの依存設定

既存の設定は[nagi.toml](projects.md)を参照してください。pathはmanifest基準で解決し、生成先を変えても同じcrateを参照します。`check`・`lower`・`symbols`はCargoを呼びません。

Cargoは依存経路ごとにfeaturesを合成します。生成先の既存Cargo.lockは保持しますが、path依存のソース内容は固定しません。`nagic build --locked`は未対応です。

## ランタイムを機能ごとに選ぶ

**未実装の案です。** HTTP・JSON・SQLite等の依存を、アプリに必要な機能へ分けます。現在はDBを使わない標準HTTPアプリでも、ランタイムのSQLite依存を含みます。

Serde・行変換・公開型・エラー変換の生成も切り替える必要があります。全関数を出力する段階では、入口から直接呼ぶ関数だけを見て依存を外せません。既存Rust adapterとの互換性も確認します。

## 不透明な型と非同期処理

**追加する資源型の契約案です。** 現在のclassはデータ用で、native clientや接続そのものを保持する型の代わりにはしません。

| 決める契約 | 検討する内容 |
|---|---|
| 型の識別 | Nagiの型とRust型の対応。同名でも提供元が違えば区別する |
| 所有・共有 | moveを基本に、borrow・clone・共有を型ごとに明示する |
| 借用 | ownerをawait中も保持する。read-only viewと排他的操作を混同しない |
| スレッド | RustのSend／Sync・traitはrustcで検証する。無条件に付与しない |
| 終了 | 明示的な非同期closeと、drop後に残る処理の責任を決める |

Dropではawaitできません。futureの破棄で呼出側が待つのをやめても、受理済みのDB処理や外部への送信が取り消される保証はありません。一般的なtraitや新しい可変長引数の文法は、この境界に必要かを確認してから検討します。

## 汎用DB APIの前提

**別driver・共通の使い方にする方針です。新APIは未実装です。** SQLiteとPostgreSQLは別module・別資源型とし、SQL方言・型・transactionの違いを保持します。module名は未決です。

| 部分 | 先に決める契約 |
|---|---|
| 引数 | 任意個の型付き値、型付きNULL、str・bytes・viewの保持規則 |
| 行 | field名・列順・NULL・整数範囲・decode失敗 |
| 操作・エラー | execute・one・all等の形、対象なし、接続・待機・SQL・decode失敗 |
| transaction | 一接続を専有する。commit／rollbackで呼出側のhandleを消費し、取消後のcleanup責任を決める |
| pool | cloneを含む閉鎖状態、新規取得の停止、実行中の操作とcleanupを何まで待つか |

transaction内の操作をlibrary側で直列化することと、並行利用をNagiの`check`で禁止することは別です。後者にはfutureの寿命まで追う追加の検査が必要です。どこまで静的に保証するかは未決です。

commitの結果を確認できない場合は、rollback済みや再実行可能とは扱いません。未送信、DBによる拒否、送信後の応答喪失をdriver側で区別する契約を検討します。

SQLは自動翻訳せず、poolの別接続へBEGIN／COMMITを送る方式も使いません。SQL/schemaの静的検査と実データの型・NULL検査は分けます。通常の`check`は現在SQLを検査しません。

## HTTP基盤の比較

Axum／Towerを第一候補に、現行の標準HTTPと比較します。**採用は未決で、比較は未実施です。** AxumもHyperを使うため、同じ接続管理の内側でRouter／middlewareを比較し、listener・停止処理の変更は別に評価します。

公開API、受付容量、本文・header制限、期限、panic応答、停止条件を揃えます。処理量・遅延・CPU・メモリ、過負荷からの回復と保守負担を確認して判断します。既存の[測定](http-stdlib-performance.md)を、この採用比較の結果とは扱いません。

送信HTTP clientも未実装です。reqwest等の再利用を検討しますが、応答statusと通信失敗、受信上限、期限、資源の終了を先に決める必要があります。

## 実装する順序

既存の型・所有権の整合性、native資源の契約、SQLiteでの検証、PostgreSQL対応の順に進めます。HTTP基盤の比較は別の評価として扱い、APIを維持できるか確認します。

旧import・組み込み・Rust連携・Lowの互換性を保ち、既存例とエディターで確認します。依存を減らす効果は、Cargo features・ビルド時間・出力容量で測ります。

参考: [Cargo依存指定](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html)、[features](https://doc.rust-lang.org/cargo/reference/features.html)、[Send](https://doc.rust-lang.org/std/marker/trait.Send.html)／[Sync](https://doc.rust-lang.org/std/marker/trait.Sync.html)、[Drop](https://doc.rust-lang.org/std/ops/trait.Drop.html)、[Axum](https://docs.rs/axum/0.8.9/axum/)、[Tower](https://docs.rs/tower/latest/tower/)。
