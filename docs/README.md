# Nagi Docs

Nagiの入門ガイドとリファレンスです。書き方や関数の仕様を調べる場合は、下の[言語リファレンス](#言語リファレンス)から選べます。

## 初めて使う

次の順に進めてください。

1. [準備と最初の実行](getting-started.md)でインストールし、Hello Worldを動かす。
2. [コードを書きながら学ぶ](language-guide.md)で変数・関数・配列・エラー処理を覚える。
3. [HTTPとHTML](http.md)でAPIを作り、[SQLite](database.md)でデータを保存する。

アプリには`.nagi`ファイルを使います。コードを保存したら、`nagic run ファイル.nagi`で実行できます。[VS Codeの使い方](editor.md)もあります。

## 言語リファレンス

| 調べたいこと | ページ |
|---|---|
| 文法、演算子、関数の定義 | [文法の早見表](syntax.md) |
| 組み込み関数の引数・戻り値・制約 | [組み込み関数](builtins.md) |
| 型の種類と型注釈 | [型と推論](types.md) |
| 関連するデータをまとめる | [class](classes.md) |
| 値を渡す、借りる、コピーする | [所有権](ownership.md)、[view](view-and-zero-copy.md) |
| 失敗を返す、成功・失敗を分ける | [エラー処理](error-handling.md) |

## アプリを作る

| やりたいこと | ページ |
|---|---|
| JSONを読み書きする | [JSON](json.md) |
| 複数ファイルに分ける、Rustを呼ぶ | [importとRust連携](modules-and-rust.md) |
| 入口やビルド設定を保存する | [nagi.toml](projects.md) |
| 非同期処理を待つ、複数の処理を始める | [asyncとscope](async.md)、[並行処理](concurrency.md) |
| 動くアプリを読む | [タスク管理デモ](../test-nagi-code/web-demo/README.md)、[Result APIサンプル](../test-nagi-code/result-api/README.md) |

入門の実行例は[examples/tutorial/](../examples/tutorial/)にあります。関数や書式の説明には、コードの一部分だけを示した例もあります。

## 仕組みと開発状況

[Nagiについて](introduction.md)、[Low](low-language.md)、[メモリの扱い](memory-model.md)、[コンパイラの構成](compiler-internals.md)、[他の言語との連携](ffi.md)、[今後の開発](roadmap.md)を参照してください。

[actor](actor.md)、[workerの再起動](supervisor.md)、[キュー](queue.md)は検証用の実装です。性能を調べる場合は、[測定方法](performance.md)、[測定結果](../PERFORMANCE.md)、[通信の負荷試験](http-capacity.md)を確認してください。

Docsは現在の0.1系を説明しています。未対応の機能は各ページに記載しています。
