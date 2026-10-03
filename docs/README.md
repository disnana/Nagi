# Nagi Docs

初めて使うなら[インストール](getting-started.md)、書き方を調べるなら[文法](syntax.md)・[組み込み関数](builtins.md)へ。

## 初めて使う

1. [準備と最初の実行](getting-started.md)でインストールし、Hello Worldを動かす。
2. [コードを書きながら学ぶ](language-guide.md)で変数・関数・配列・エラー処理を覚える。
3. [HTTPとHTML](http.md)でAPIを作り、[SQLite](database.md)でデータを保存する。

[VS Codeの使い方](editor.md)も確認できます。

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
| JSONを読み書きする | [JSON](json.md) |
| 複数ファイルに分ける、Rustを呼ぶ | [importとRust連携](modules-and-rust.md) |
| 自作の共通コードやRustのcrateを使う | [ライブラリとRustの資産](libraries.md) |
| 入口やビルド設定を保存する | [nagi.toml](projects.md) |
| 自分のRust crateをアプリから使う | [ローカルライブラリのサンプル](../test-nagi-code/rust-library/README.md) |
| 非同期処理を待つ、複数の処理を始める | [asyncとscope](async.md)、[並行処理](concurrency.md) |
| 動くアプリを読む | [サンプルプロジェクト一覧](library-examples.md) |

動かせる入門例は[examples/tutorial/](../examples/tutorial/)にあります。リファレンスにはコードの断片も載せています。

## 仕組みと開発状況

[Nagiについて](introduction.md) · [Low](low-language.md) · [メモリ](memory-model.md) · [コンパイラ](compiler-internals.md) · [他の言語との連携](ffi.md) · [開発予定](roadmap.md)

[actor](actor.md)、[workerの再起動](supervisor.md)、[キュー](queue.md)は検証用の実装です。性能を調べる場合は、[測定方法](performance.md)、[測定結果](../PERFORMANCE.md)、[通信の負荷試験](http-capacity.md)を確認してください。

現在の0.1系が対象です。未対応の機能は各ページに記載しています。
