# ライブラリとRust連携のサンプル

10のプロジェクトで、共通コードのimport、独自エラー、Rust crate、標準HTTP、自作Axumサーバー、Supervisor、Lowの関数差し替えを試します。

[一覧と起動手順](../../docs/library-examples.md) · [共通コードの構成](../../docs/libraries.md) · [English](README.en.md)

リポジトリのルートから実行します。Nagi 0.1.9、Rust/Cargoとアプリをビルドできる環境が必要です。

```sh
nagic run --project test-nagi-code/library-examples/rust-json
```

料金計算の2つのアプリは`shared/`も読み込みます。このディレクトリ一式を取得してください。各プロジェクトのREADMEに入力、出力、制約があります。
