# Nagi Docs

初めて使うなら[インストール](getting-started.md)から[最初のCLIアプリ](first-app.md)まで順に進めます。言語全体の道筋は[言語機能索引](#言語機能索引)、書式や関数を引くには[文法](syntax.md)・[組み込み関数](builtins.md)へ。

このDocsは2026-10-07に公開したNagi 0.1.11をcompiler基準版とし、VS Code拡張0.1.13とJetBrainsプラグイン0.1.1は別componentとして扱います。公式サイトは通常mainから生成するため、まだ公開されていない変更も含むことがあります。JetBrains版の配布版とIDE別ZIPの選び方は[導入方法](../editors/jetbrains-nagi/README.md)を参照してください。版ごとの変更は[Changelog](../CHANGELOG.md)、互換性の変更は[0.1.11移行ガイド](migration-0.1.11.md)にあります。

目的と現在の範囲は[Nagiについて](introduction.md)へ。以下のリファレンスは実装済みAPIの使い方と制限を示します。未実装の案は[設計](library-design.md)・[開発予定](roadmap.md)に分けています。

## 初めて使う

1. [準備と最初の実行](getting-started.md)でcompilerを用意し、Hello Worldを動かす。
2. [コードを書きながら学ぶ](language-guide.md)で値・関数・配列・borrow・Resultを試す。
3. [最初の小さなCLIアプリ](first-app.md)で、入力・型付きの失敗・境界値の手動確認を一続きで行う。
4. [HTTP](http.md)でAPIを作り、[SQLite](database.md)でデータを保存する。

[VS Codeの使い方](editor.md)と[JetBrains版の導入・Release ZIPの選び方](../editors/jetbrains-nagi/README.md)も確認できます。

## 言語機能索引

まず用例を動かし、期待する結果を確認してから詳細ページで条件や制約を調べます。下の列は、各機能の利用目的、動く例と結果、誤りの直し方、使い分けを探す入口です。コード片は完全なプログラムとは限らないため、`nagic run`の手順がある節か、右端のサンプルを確認してください。

| 機能 | 使いどころ・動かす例と結果 | 制約・よくある誤りと直し方 | 使い分けと詳細 |
|---|---|---|---|
| 値、型注釈、代入・再代入 | [入門 §1](language-guide.md#1-値と型)で数値を更新し`11`を表示 | 型は最初の値から決まり、別の型へは再代入できない | [型](types.md)、[値と変数](syntax.md#値と変数) |
| 関数、引数、return、関数値 | [入門 §2](language-guide.md#2-関数を定義する)で`42`を返す | 引数と戻り値の型を書き、実行文は`main`から呼ぶ | [関数構文](syntax.md#関数とreturn)、[関数型](types.md#関数を値として渡す) |
| if、match、for、while、演算子 | [入門 §3](language-guide.md#3-配列class分岐繰り返し)で`OK`と反復結果を表示 | bool以外を条件にできない。`break`、`continue`、`elif`は未対応 | [分岐・loop・演算子](syntax.md#分岐とループ)、[match](error-handling.md#成功と失敗を分ける) |
| List、class、field、enum | [入門 §3](language-guide.md#3-配列class分岐繰り返し)と[class例](classes.md)で合計・field値を表示 | Listは要素型を揃える。classは名前付きfieldで作り、method・継承・field再代入は未対応 | [型とenum](types.md#enumで種類を分ける)、[class](classes.md) |
| nullable `T?` / `Option[T]` | [値がない場合の例](error-handling.md#値がない場合を扱う)で`Some`と`None`を処理 | 両方を`match`する。`unwrap`や`if value is not None`の型絞り込みはない | [型](types.md)、[match構文](syntax.md#resultasyncscope) |
| `Result[T, E]`、`try`、`match`、独自Error | [最初のCLI](first-app.md)で入力失敗を伝え、[入門 §5](language-guide.md#5-失敗する処理はresultで返す)で回復例を読む | `try`はErrを呼出元へ返す。局所回復には`match`でOk/Err両方を書く | [失敗の種類と例](error-handling.md)、[組み込み関数](builtins.md#変換成功失敗) |
| ownership、通常代入での明示move、copy | [入門 §4](language-guide.md#4-読むだけならviewで借りる)で受渡し後の出力を確認 | 非Copyローカルの通常代入は`move(...)`を使う。move後の元値は使えない | [代入・move・誤り](ownership.md#代入と明示move)、[移行](migration-0.1.11.md) |
| borrowと`view` | [view例](view-and-zero-copy.md)で文字列の一部を読み、copyした値と比較 | 元データより長く保存・返却できない。一時値のviewを保存しない | [borrowと検査範囲](ownership.md#借用と検査の範囲)、[view](view-and-zero-copy.md) |
| `shared`、`owned`、Copy | [組み込み関数](builtins.md#共有と型のサイズ)のshare/clone例 | `shared`は任意の型を自動でthread-safeにはしない。`owned[T]`は未完成 | [型](types.md#未対応の型操作)、[メモリ](memory-model.md) |
| ファイル、import、module alias | [入門 §6](language-guide.md#6-ファイルを分ける)で2ファイルから関数を使う | 相対パスと登録済みstd moduleが対象。循環importや一般package探索は未対応 | [module・Rust連携](modules-and-rust.md) |
| `async`関数、`await` | [async例](async.md#待ち終わった結果を使いたい)で待機後に表示 | `await`自体はtask/threadを作らない。Futureを変数へ保存する形式は未対応 | [async](async.md)、[CPU並行処理](concurrency.md) |
| `scope`、`spawn`、Task結果handle | [scope例](async.md#待っている間に別の処理も進めたい)で子の終了を待つ | 正常出口には子の終了規則がある。Taskはscopeの外へescapeできず、受取は一度 | [Task結果・Err・故障](task-handles.md)、[scope](async.md) |
| Actor、Supervisor | [監視付きservice](../test-nagi-code/library-examples/supervised-service/README.md)で登録・呼出・停止を試す | 同一process内のAPI。再起動、業務Err、call失敗、shutdownの条件を確認する | [actor](actor.md)、[Supervisor](supervisor.md)、[型API](actor-reference.md) |
| HighとLow | [High/Lowの実行例](low-language.md#lowだけで書いて実行する) | Lowも同じ型・ownership規則を使う。pointer/unsafe/C ABIを使う言語層ではない | [HighとLow](low-language.md)、[Rust連携](modules-and-rust.md) |
| 組み込み関数 | [関数一覧](builtins.md)から引数と戻り値を探す。主要例は各節と[動くsample](library-examples.md)にある | 引数の型、borrow/consume、同期/async、実行時条件が関数ごとに異なる | [全組み込み関数](builtins.md)、[HTTP](http.md)、[SQLite](database.md) |

各ページは現在の実装と版の範囲に合わせ、用途、コード例と結果、条件・制約、誤った書き方と修正、近い機能との使い分けを載せるか、対応する節・sampleへ案内します。質問が複数の機能にまたがる場合は[サンプルプロジェクト一覧](library-examples.md)も確認してください。

## 言語リファレンス

| 調べたいこと | ページ |
|---|---|
| 文法、演算子、関数の定義 | [文法の早見表](syntax.md) |
| 組み込み関数の引数・戻り値・制約 | [組み込み関数](builtins.md) |
| 型の種類と型注釈 | [型と推論](types.md) |
| 名前付きのデータ | [class](classes.md) |
| 値を渡す、借りる、コピーする | [所有権](ownership.md)、[view](view-and-zero-copy.md) |
| 失敗を返す・処理する | [エラー処理](error-handling.md) |

## アプリを作る

| やりたいこと | ページ |
|---|---|
| DBなしのHTTP、ヘッダー、応答status | [HTTP](http.md)、[APIリファレンス](http-server.md) |
| JSONを読み書きする | [JSON](json.md) |
| 旧SQLite APIとSQLの事前検査 | [SQLite](database.md)、[SQLの事前検査](sql-check.md) |
| 型付きparameters・Pool・transactionを使う | [SQLite PoolとTransaction（開発source・未リリース）](sqlite-pool.md) |
| 複数ファイルに分ける、Rustを呼ぶ | [importとRust連携](modules-and-rust.md) |
| 自作の共通コードやRustのcrateを使う | [ライブラリとRustの資産](libraries.md) |
| 入口やビルド設定を保存する | [nagi.toml](projects.md) |
| 自分のRust crateをアプリから使う | [ローカルライブラリのサンプル](../test-nagi-code/rust-library/README.md) |
| 非同期処理を待つ、複数の処理を始める | [asyncとscope](async.md)、[並行処理](concurrency.md) |
| 状態を持つ処理へメッセージを送り、再起動・停止を管理する | [actor](actor.md)、[Supervisor](supervisor.md)、[APIリファレンス](actor-reference.md) |
| 動くアプリを読む | [サンプルプロジェクト一覧](library-examples.md) |
| 型・モジュール・呼び出しを図にする | [コードマップ](code-map.md) |

動かせる入門例は[examples/tutorial/](../examples/tutorial/)にあります。リファレンスにはコードの断片も載せています。

入門のコードは現行の書き方です。明示moveとTask結果handleはNagi 0.1.11で公開済みです。利用するcompilerが0.1.11以降であることを`nagic --version`で確認してください。[移行ガイド](migration-0.1.11.md)と[Task結果handle](task-handles.md)に使い方をまとめています。actorのshared messageは将来の設計対象です。

新しい[SQLite Pool/Tx](sqlite-pool.md)は開発sourceのみで、0.1.11には含まれません。

## 仕組みと開発状況

[Nagiについて](introduction.md) · [設計判断](../DESIGN.md) · [Low](low-language.md) · [メモリ](memory-model.md) · [コンパイラ](compiler-internals.md) · [他の言語との連携](ffi.md) · [開発予定](roadmap.md)

本体のfileを探し、小さい変更を実行・検証してPRにする手順は[初めての貢献](contributing.md)へ。

`std.actor`はNagi 0.1.8から使える標準ライブラリです。[サンプル](../test-nagi-code/library-examples/supervised-service/README.md)で登録・呼び出し・停止を試せます。旧actor／Supervisorの組み込み関数と[キュー](queue.md)は検証用APIです。性能を調べる場合は、[測定方法](performance.md)、[測定結果](../PERFORMANCE.md)、[通信の負荷試験](http-capacity.md)を確認してください。

現在のソースと0.1系が対象です。版ごとの変更は[変更履歴](../CHANGELOG.md)、未対応の機能は各ページに記載しています。

- [認証・認可（未リリースSF01）](security.md)と[0.2.0移行](migration-0.2.0.md)
