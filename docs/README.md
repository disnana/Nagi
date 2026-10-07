# Nagi Docs

初めて使うなら[インストール](getting-started.md)、書き方を調べるなら[文法](syntax.md)・[組み込み関数](builtins.md)へ。

このDocsはNagi 0.1.11をcompiler基準版とし、VS Code拡張0.1.13とJetBrainsプラグインは別componentとして扱います。公式サイトは通常mainから生成するため、まだ公開されていない変更も含むことがあります。0.1.11の公開有無は公式Release記録で確認してください。JetBrains版の配布版とIDE別ZIPの選び方は[導入方法](../editors/jetbrains-nagi/README.md)を参照してください。版ごとの変更は[Changelog](../CHANGELOG.md)、互換性の変更は[0.1.11移行ガイド](migration-0.1.11.md)にあります。

目的と現在の範囲は[Nagiについて](introduction.md)へ。以下のリファレンスは実装済みAPIの使い方と制限を示します。未実装の案は[設計](library-design.md)・[開発予定](roadmap.md)に分けています。

## 初めて使う

1. [準備と最初の実行](getting-started.md)でインストールし、Hello Worldを動かす。
2. [コードを書きながら学ぶ](language-guide.md)で、Pythonとの比較、短いNagi例、結果、間違いの直し方を順に読む。変数・関数・配列・値の渡し方・エラー処理を試せる。
3. [HTTPとHTML](http.md)でAPIを作り、[SQLite](database.md)でデータを保存する。

[VS Codeの使い方](editor.md)と[JetBrains版の導入・Release ZIPの選び方](../editors/jetbrains-nagi/README.md)も確認できます。

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

入門のコードは現行の書き方です。明示moveとTask結果handleはNagi 0.1.11で公開済みです。利用するcompilerが0.1.11以降であることを確認してください。[移行ガイド](migration-0.1.11.md)と[Task結果handle](task-handles.md)に使い方をまとめています。actorのshared messageは将来の設計対象です。

## 仕組みと開発状況

[Nagiについて](introduction.md) · [設計判断](../DESIGN.md) · [Low](low-language.md) · [メモリ](memory-model.md) · [コンパイラ](compiler-internals.md) · [他の言語との連携](ffi.md) · [開発予定](roadmap.md)

`std.actor`はNagi 0.1.8から使える標準ライブラリです。[サンプル](../test-nagi-code/library-examples/supervised-service/README.md)で登録・呼び出し・停止を試せます。旧actor／Supervisorの組み込み関数と[キュー](queue.md)は検証用APIです。性能を調べる場合は、[測定方法](performance.md)、[測定結果](../PERFORMANCE.md)、[通信の負荷試験](http-capacity.md)を確認してください。

現在のソースと0.1系が対象です。版ごとの変更は[変更履歴](../CHANGELOG.md)、未対応の機能は各ページに記載しています。
