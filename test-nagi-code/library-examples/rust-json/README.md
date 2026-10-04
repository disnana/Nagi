# serde_jsonをNagiの型で使う

[English](README.en.md)

Rustの`serde_json`でJSONを読み、Nagiで宣言した`JsonRecord`を返す小さなプロジェクトです。Nagiからは`extern def`を通して呼び出し、Rust側では生成された型を`super::JsonRecord`として使います。ライブラリ固有の型をNagiの公開APIへ追加せず、アダプターの中に実装を置けます。

型付きJSONの読み書きは、標準の`json_decode`／`json_encode`でも書けます。この例の目的はRust crateの関数をNagiの型へ橋渡しすることです。

入力は`view[str]`で借りるので、同じ文字列を2回解析し、その後も表示できます。返されたレコードの文字列フィールドはデータを所有します。不正なJSONとフィールドの型違いは`Result`の失敗になり、Nagiの`match`で処理します。

## 実行する

Nagiが`nagic`としてPATHにあり、Rust/CargoとCビルド環境が使える状態で、リポジトリのルートから実行します。初回の依存取得にはネットワーク接続が必要です。依存がキャッシュ済みなら取得は不要です。

```sh
nagic check --project test-nagi-code/library-examples/rust-json
nagic run --project test-nagi-code/library-examples/rust-json
```

Rustのビルドログとは別に、プログラムは次を出力します。

```text
Nagi
2
{"label":"Nagi","count":2}
malformed -> invalid
wrong type -> invalid
rust-json: OK
```

不正入力は意図した確認ケースです。両方の失敗を処理した後、プログラムは成功で終了します。期待と異なる結果は`assert_true`で検出します。

## ファイルと範囲

- [serde-record.nagi](serde-record.nagi)：型、Rust関数の宣言、借用とResultの確認。
- [native.rs](native.rs)：`serde_json::from_str`と、ライブラリのエラーからNagiの`Error`への変換。
- [nagi.toml](nagi.toml)：エントリーファイル、アダプター、`serde_json = "1.0"`の依存指定。

`check`はNagiの宣言・型・所有権を確認します。Rust本体とcrateのAPIが宣言に合うかどうかは`run`のビルドで確認します。レコードの余分なフィールド、必須フィールドの不足、型違いも拒否します。この例は同じRustビルドへ組み込む連携で、動的なライブラリ読み込みやJSONの汎用オブジェクトAPIは提供しません。

仕様は[型付きRust連携](../../../docs/modules-and-rust.md)を参照してください。
