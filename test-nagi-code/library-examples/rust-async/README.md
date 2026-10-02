# Rustの非同期処理をNagiからawaitする

[English](README.en.md)

`extern async def`からRustの非同期アダプターを呼び出すプロジェクトです。アダプターはTokioのタイマーで実際に待機し、整数を2倍にした値を`Result[i64, Error]`として返します。Nagiの`try await`で成功値を受け取り、失敗は`match await`で処理します。

## 実行する

Nagiが`nagic`としてPATHにあり、Rust/CargoとCビルド環境が使える状態で、リポジトリのルートから実行します。初回の依存取得にはネットワーク接続が必要です。依存がキャッシュ済みなら取得は不要です。

```sh
nagic check --project test-nagi-code/library-examples/rust-async
nagic run --project test-nagi-code/library-examples/rust-async
```

Rustのビルドログとは別に、プログラムは次を出力します。

```text
42
invalid delay -> invalid
overflow -> invalid
rust-async: OK
```

成功ケースは10ミリ秒のタイマーを待ちます。負の待機時間とi64の範囲を超える計算は、待機する前に失敗として返します。両方の失敗を処理した後、プログラムは成功で終了します。期待と異なる結果は`assert_true`で検出します。

## ファイルと範囲

- [tokio-timer.nagi](tokio-timer.nagi)：非同期関数の宣言と、成功・失敗の確認。
- [native.rs](native.rs)：入力検証、`checked_mul`、`tokio::time::sleep`。
- [nagi.toml](nagi.toml)：エントリーファイル、アダプター、`tokio = "1.48"`の依存指定。

アダプターはNagiが用意するTokioランタイム上で動きます。ランタイムを新しく作ったり、処理の内側で`block_on`を呼んだりしません。ブロックするsleepやファイルI/Oも使いません。待機時間はこの例で0〜1000ミリ秒に制限しています。タイマーの指定時間は、実際の完了時刻や性能を保証する値ではありません。

`check`はNagi側の型とawaitを確認します。Rust本体とTokioのAPIは`run`のビルドで確認します。この例はタイマーの連携を扱い、キャンセル・タスク管理・I/O全般のAPIは追加しません。

仕様は[型付きRust連携](../../../docs/modules-and-rust.md)を参照してください。
