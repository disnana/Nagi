# Nagiの書き方

初めて書く場合は、**[準備と最初の実行](getting-started.md) → [コードを書きながら学ぶ](language-guide.md)** の順に読んでください。アプリを書くにはHigh（`.nagi`）を使います。Lowを先に覚える必要はありません。

## まず読む

| やりたいこと | ページ | 分かること |
|---|---|---|
| Hello Worldを動かす | [準備と最初の実行](getting-started.md) | Windows / Linuxのコマンド、ファイル作成、ビルド、VS Code |
| VS Codeで型と補完を使う | [エディターの操作例](editor.md) | 変数の型ホバー、フィールド補完、引数ヒント、F12 |
| 文法を順番に覚える | [コードを書きながら学ぶ](language-guide.md) | 変数、関数、配列、class、view、Result、import |
| 書き方をすぐ調べる | [文法の早見表](syntax.md) | 書式、演算子、Pythonと異なるところ |
| 組み込み関数を探す | [よく使う関数](builtins.md) | 引数、戻り値、await / tryが必要な場面 |
| 小さなサイトとAPIを作る | [HTTPとHTML](http.md) | 動くサーバー、JSONの送受信、HTMLの表示 |
| 動くアプリを読む | [タスク管理デモ](../test-nagi-code/web-demo/README.md) | ブラウザー画面、CRUD API、SQLite、exe配布 |
| APIの失敗を分ける | [Result APIサンプル](../test-nagi-code/result-api/README.md) | match、400・404・500、代替データへの回復 |

## 必要になったら読む

| 話題 | ページ |
|---|---|
| 型注釈、数値、nullable | [型](types.md) |
| データのまとまりを定義する | [class](classes.md) |
| 値を渡すと再利用できない理由 | [所有権](ownership.md)、[viewとコピー](view-and-zero-copy.md) |
| 失敗を返す、try・matchを使う | [エラー処理](error-handling.md) |
| 複数ファイル、Rustのライブラリを使う | [importとRust連携](modules-and-rust.md) |
| 入口・Rust依存・Lowの設定をまとめる | [nagi.tomlとプロジェクト](projects.md) |
| JSONを読み書きする | [JSON](json.md) |
| データを保存する | [SQLite](database.md) |
| 非同期処理、子taskを待つ | [asyncとscope](async.md)、[並行処理](concurrency.md) |
| 生成Lowを読む、関数を差し替える | [Low](low-language.md) |

## 設計と実装の資料

これらは書き方を覚えたあとに読む資料です。

- [目的と実装範囲](introduction.md)、[今後の開発](roadmap.md)
- [メモリモデル](memory-model.md)、[コンパイラ内部](compiler-internals.md)、[FFI](ffi.md)
- [actor](actor.md)、[Supervisor](supervisor.md)、[queue](queue.md)：現在は試験用の組み込み関数
- [性能の読み方](performance.md)、[測定結果](../PERFORMANCE.md)

## サンプルの読み方

コードブロックはNagiのコードです。Pythonに似た字下げを使いますが、`python`コマンドでは実行できません。`nagic run ファイル.nagi`を使います。

入門の完成ファイルは [examples/tutorial/](../examples/tutorial/) にあります。[文法の早見表](syntax.md)や関数一覧にある短いコードは、関数の中に書く断片も含みます。ページ内で「完全なコード」と示す例は、そのままファイルに保存して実行できます。

この文書は現在の0.1実装を説明します。未実装の機能は各ページの末尾などで区別しています。`import "file.nagi"`とResultの`match`は使えます。`import package as alias`、classのmethod、nullableのmatchは未対応です。
