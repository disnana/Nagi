# 他の言語との連携

現在はRustの関数をNagiから呼べます。`@rust`と`extern def`で関数を宣言し、RustのファイルやCargoの依存をビルドに含めます。書き方は[importとRust連携](modules-and-rust.md)を参照してください。

HighとLowの同じ型は、同じRust型へ変換します。同じビルド内の関数呼び出しでは、値をJSONなどへ変換し直す必要はありません。

## Cとの連携

NagiやLowから任意のC関数を直接呼ぶ構文は未対応です。組み込みSQLiteでは、Rustの`rusqlite`と`libsqlite3-sys`がCとの接続を担当しています。

Cに公開するための固定の型配置や、メモリの確保・解放を受け渡すABIも未定義です。Nagiのclassや文字列を、そのままCの値として扱うことはできません。
