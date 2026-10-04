# Highの呼び出しを残してLowを差し替える

整数列の二乗和を計算します。アプリはHighで書き、`sum_squares`の実装を`kernel.low`に置き換えます。引数と戻り値は同じです。

このディレクトリで実行します。Rust/CargoとNagiアプリをビルドできる環境が必要です。

```sh
nagic check
nagic run
```

出力は次のとおりです。

```text
30
4
```

`nagi.toml`の`native`がLowを指定しています。元のHighだけを動かす場合は、プロジェクト設定を使わずに実行します。

```sh
nagic run low_kernel.nagi --no-project
```

両方とも同じ結果です。配列を借りているため、呼び出した後も`len(samples)`を使えます。

Lowは同じ型・所有権検査を受け、最終的にRustへ変換されます。ここでは関数を置き換える仕組みを示しています。Low側も同じ計算であり、速度の改善を測った例ではありません。固定幅整数なので、大きな入力に対するoverflowの扱いは別途設計してください。

[English](README.en.md) · [HighとLow](../../../docs/low-language.md)
