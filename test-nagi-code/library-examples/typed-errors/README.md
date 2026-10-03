# 独自エラーを使うCLI

数量を1〜1000000の範囲で検証し、成功なら2倍の値を表示します。失敗は`QuantityError`にまとめ、表示する場所で種類ごとのメッセージに変えます。DBやHTTPは使いません。

enum対応のNagiで、このディレクトリから実行します。`run`にはRust/Cargoと、お使いのOSでアプリをビルドできる環境が必要です。

```sh
nagic check
nagic lower
nagic run
```

```text
42
quantity must be between 1 and 1000000
quantity must be a number
```

`validation.nagi`は組み込みのparseエラーを`NotNumber(cause: Error)`へ明示的に包み、原因を保持します。`doubled`は同じエラー型を`try`で伝播します。`message`は全種類をmatchし、内部のcauseを表示文へ含めません。`InputError`と`validation.QuantityError`は同じ型です。

enumと組み込みErrorを含む型のJSON変換は未対応です。標準HTTPのハンドラーは、引き続き`Result[..., Error]`を返します。

[English](README.en.md) · [エラー処理](../../../docs/error-handling.md)
