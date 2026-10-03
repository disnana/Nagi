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
| DBなしのHTTP、ヘッダー、応答status | [HTTP](http.md)、[APIリファレンス](http-server.md) |
| JSONを読み書きする | [JSON](json.md) |
| 複数ファイルに分ける、Rustを呼ぶ | [importとRust連携](modules-and-rust.md) |
| 自作の共通コードやRustのcrateを使う | [ライブラリとRustの資産](libraries.md) |
| 入口やビルド設定を保存する | [nagi.toml](projects.md) |
| 自分のRust crateをアプリから使う | [ローカルライブラリのサンプル](../test-nagi-code/rust-library/README.md) |
| 非同期処理を待つ、複数の処理を始める | [asyncとscope](async.md)、[並行処理](concurrency.md) |
| 状態を持つ処理へメッセージを送り、再起動・停止を管理する | [actor](actor.md)、[Supervisor](supervisor.md)、[APIリファレンス](actor-reference.md) |
| 動くアプリを読む | [サンプルプロジェクト一覧](library-examples.md) |
| 型・モジュール・呼び出しを図にする | [コードマップ](code-map.md) |

動かせる入門例は[examples/tutorial/](../examples/tutorial/)にあります。リファレンスにはコードの断片も載せています。

## 仕組みと開発状況

[Nagiについて](introduction.md) · [Low](low-language.md) · [メモリ](memory-model.md) · [コンパイラ](compiler-internals.md) · [他の言語との連携](ffi.md) · [開発予定](roadmap.md)

`std.actor`はNagi 0.1.8から使える標準ライブラリです。[サンプル](../test-nagi-code/library-examples/supervised-service/README.md)で登録・呼び出し・停止を試せます。旧actor／Supervisorの組み込み関数と[キュー](queue.md)は検証用APIです。性能を調べる場合は、[測定方法](performance.md)、[測定結果](../PERFORMANCE.md)、[通信の負荷試験](http-capacity.md)を確認してください。

現在のソースと0.1系が対象です。版ごとの変更は[変更履歴](../CHANGELOG.md)、未対応の機能は各ページに記載しています。
