# Nagi Docs

初めて使うなら[インストール](getting-started.md)、書き方を調べるなら[文法](syntax.md)・[組み込み関数](builtins.md)へ。

このDocsのコード例はNagi 0.1.10とVS Code拡張0.1.13を基準にしています。公式サイトは通常mainから生成するため、未リリースの変更も含まれます。版ごとの変更と`Unreleased`の内容は[Changelog](../CHANGELOG.md)で確認できます。

目的と現在の範囲は[Nagiについて](introduction.md)へ。以下のリファレンスは実装済みAPIの使い方と制限を示します。未実装の案は[設計](library-design.md)・[開発予定](roadmap.md)に分けています。

## 初めて使う

1. [準備と最初の実行](getting-started.md)でインストールし、Hello Worldを動かす。
2. [コードを書きながら学ぶ](language-guide.md)で、Pythonとの比較、短いNagi例、結果、間違いの直し方を順に読む。変数・関数・配列・値の渡し方・エラー処理を試せる。
3. [HTTPとHTML](http.md)でAPIを作り、[SQLite](database.md)でデータを保存する。

[VS Codeの使い方](editor.md)と[JetBrains版の導入方法](../editors/jetbrains-nagi/README.md)も確認できます。

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
| SQLiteのSQLをschemaと照合する | [SQLの事前検査](sql-check.md) |
| 複数ファイルに分ける、Rustを呼ぶ | [importとRust連携](modules-and-rust.md) |
| 自作の共通コードやRustのcrateを使う | [ライブラリとRustの資産](libraries.md) |
| 入口やビルド設定を保存する | [nagi.toml](projects.md) |
| 自分のRust crateをアプリから使う | [ローカルライブラリのサンプル](../test-nagi-code/rust-library/README.md) |
| 非同期処理を待つ、複数の処理を始める | [asyncとscope](async.md)、[並行処理](concurrency.md) |
| 状態を持つ処理へメッセージを送り、再起動・停止を管理する | [actor](actor.md)、[Supervisor](supervisor.md)、[APIリファレンス](actor-reference.md) |
| 動くアプリを読む | [サンプルプロジェクト一覧](library-examples.md) |
| 型・モジュール・呼び出しを図にする | [コードマップ](code-map.md) |

動かせる入門例は[examples/tutorial/](../examples/tutorial/)にあります。リファレンスにはコードの断片も載せています。

入門のコードは現行の書き方です。明示moveの新規則、spawn結果handle、actorのshared messageは[採用した設計方向](../DESIGN.md#値失敗taskについて採用する方針)に分けており、まだ使える構文ではありません。

## 仕組みと開発状況

[Nagiについて](introduction.md) · [設計判断](../DESIGN.md) · [Low](low-language.md) · [メモリ](memory-model.md) · [コンパイラ](compiler-internals.md) · [他の言語との連携](ffi.md) · [開発予定](roadmap.md)

`std.actor`はNagi 0.1.8から使える標準ライブラリです。[サンプル](../test-nagi-code/library-examples/supervised-service/README.md)で登録・呼び出し・停止を試せます。旧actor／Supervisorの組み込み関数と[キュー](queue.md)は検証用APIです。性能を調べる場合は、[測定方法](performance.md)、[測定結果](../PERFORMANCE.md)、[通信の負荷試験](http-capacity.md)を確認してください。

現在のソースと0.1系が対象です。版ごとの変更は[変更履歴](../CHANGELOG.md)、未対応の機能は各ページに記載しています。
