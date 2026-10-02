# 自作ライブラリとRustの資産を使う

Nagiでアプリの処理を書き、共通の関数を別ファイルにまとめたり、Rustのライブラリを呼んだりできます。HTTPやDBをすべて組み込み関数で実装する必要はありません。

## 共通の処理を複数のアプリで使う

[料金計算CLI](../test-nagi-code/library-examples/foundation-cli/README.md)と[JSONレポート](../test-nagi-code/library-examples/foundation-report/README.md)は、同じ料金計算ライブラリを読み込みます。共通のNagiファイルが入力の検証と公開する型を決め、Rust側に計算の実装を置いています。

```text
library-examples/
  shared/              共通のNagi宣言とRustの実装
  foundation-cli/      入力を受け取って見積もるアプリ
  foundation-report/   複数の見積もりをJSONにするアプリ
```

`import "../shared/foundation.nagi"`のように相対パスで共通ファイルを読み込みます。Rustの入口は各アプリの`nagi.toml`に指定し、そのRustファイルから共通モジュールを読み込みます。サンプルはディレクトリ一式で使ってください。

現在のimportはファイルの定義を同じ名前空間へ読み込みます。公開範囲やaliasはないので、共通関数には名前が衝突しにくい名前を付けます。Cargoのcrateとしての配布や、Nagiのパッケージ管理とは別の仕組みです。

## Rustライブラリとの境界を小さくする

[Rust連携のリファレンス](modules-and-rust.md)に、`@rust`・`extern def`・型の対応をまとめています。

Rustのcrateを使うときは、Rust側にNagiから呼ぶ関数を用意します。その関数でcrate固有の型をNagiの数値・文字列・class・Resultへ変換すると、アプリ側はRustの実装の詳細を知らずに使えます。

[JSON読込](../test-nagi-code/library-examples/rust-json/README.md)では、`serde_json`がNagiのclassに対応するJSONを読みます。入力の文字列は借り、形式が合わない場合はResultの失敗を返します。[非同期処理](../test-nagi-code/library-examples/rust-async/README.md)では、TokioのtimerをNagiからawaitします。

Nagiの`check`が検査するのは宣言と呼び出しです。Rustの関数との型の一致は`build`で検査します。Rustの任意の型、trait、genericをそのままNagiから使う仕組みではありません。Rust側のblocking処理や独自の共有状態は、そのライブラリ側で管理します。

## アプリの処理をRust側から呼ぶ

引数に同期関数の型を宣言すると、Nagiの関数をRustへ渡せます。[自作HTTP基盤](../test-nagi-code/library-examples/custom-http/README.md)はこの方法を使います。

HTTPの受付と停止はRustのAxum/Tokio、応答を作る関数はNagiが担当します。Nagiの組み込み`serve`やDbを使わずに、Rust側の基盤にアプリの処理を渡す例です。

このサーバーには、Nagiの組み込みHTTPサーバーの制限が自動で適用されません。サンプルが設定する上限はREADMEに記載しています。また、現在の生成アプリは共通ランタイムへ依存するため、Dbを呼ばないプログラムでもビルド時のSQLite依存は残ります。

## Highの実装をLowへ置き換える

[Low計算カーネル](../test-nagi-code/library-examples/low-kernel/README.md)は、Highの呼び出しを変えずに、同じ型のLow実装へ置き換えます。Rustライブラリを追加せず、Nagi内で実装を差し替える方法です。

実装をLowに移すだけで速くなるとは限りません。結果の一致を確認し、必要な箇所で速度や割り当てを測定してください。

## ライブラリ利用で今後整えること

以下は設計対象であり、現在使える設定・文法ではありません。

- Rust依存の`path`・`features`・`default-features`を指定し、手元のcrateや必要な機能だけを組み込む。
- Nagiの名前付きmodule・`as`・`from`・公開範囲を定義し、ライブラリ同士の名前の衝突を防ぐ。
- HTTP・DBを用途に応じて選べる標準ライブラリに分け、使わない依存をビルドから外す。
- 自作の接続・clientなどを扱う型について、所有権・共有・終了・asyncの取消しの規則を決める。
- DBを一般化する際は、PostgreSQLへの接続だけでなく、型付きのSQL引数・行の読込・transactionを設計する。

既存のファイルimport・extern・SQLiteの呼び出しを使うコードを保ちながら、これらの設計を確認して進めます。[具体的な設計案](library-design.md)と[サンプル一覧](library-examples.md)があります。
