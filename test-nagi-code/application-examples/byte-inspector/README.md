# バイト列を読むHTTP API

[English](README.en.md)

POSTした本文のバイト数、各バイトの合計、先頭の値をJSONで返します。空の本文の`first`は`null`です。

Nagiは`view[bytes]`をコピーせずに読み、`for`とindexで取り出した`u8`を集計します。HTTPの受信・応答は標準ライブラリが担当します。手書きRustやDBは使いません。

この例には、バイト列の反復・indexの修正を含むNagi 0.1.10以降が必要です。次のコマンドはリポジトリのソースからコンパイラをビルドして実行します。

```sh
cargo run -p nagic -- run --project test-nagi-code/application-examples/byte-inspector
```

別のターミナルから送信します。

```sh
curl --data-binary 'ABC' http://127.0.0.1:8096/bytes
```

```json
{"length":3,"total":198,"first":65}
```

Windowsでは`curl.exe`を使えます。既定のportは8096で、`NAGI_SAMPLE_PORT`で変更できます。本文上限は4096バイトです。`total`は単純な合計で、改ざんの検出や認証には使えません。

Highと保存したLowを実HTTPで検証するには、次を実行します。空の本文、0〜255の値、UTF-8、不正なUTF-8、上限内の本文、HEAD・method・未登録routeを確認します。

```sh
cargo build -p nagic
python scripts/verify_application_examples.py --compiler target/debug/nagic --only byte-inspector
```
