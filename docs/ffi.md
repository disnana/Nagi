# 他の言語との連携

現在はRustの関数をNagiから呼べます。Highで`@rust`と`extern def`を宣言し、RustのファイルやCargoの依存をビルドに含めます。手書きLowは不要です。書き方は[importとRust連携](modules-and-rust.md)を参照してください。

HighとLowの同じ型は、同じRust型へ変換します。同じビルド内の関数呼び出しでは、値をJSONなどへ変換し直す必要はありません。

この連携は同じRustビルド内の呼び出しであり、固定ABIや動的libraryの読込みではありません。Rust側の型の一致は`build`で検査します。Rustアダプターのunsafe操作や外部資源の後始末を、Nagiの`check`で保証することはできません。

## Cとの連携

NagiやLowから任意のC関数を直接呼ぶ構文は未対応です。組み込みSQLiteでは、Rustの`rusqlite`と`libsqlite3-sys`がCとの接続を担当しています。

Cに公開するための固定の型配置や、メモリの確保・解放を受け渡すABIも未定義です。Nagiのclassや文字列を、そのままCの値として扱うことはできません。
