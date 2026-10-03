# module名とfromの別名

`orders.nagi`と`archive.nagi`は、それぞれ同名の`Order`と`total`を定義します。アプリはmodule名で区別し、`orders.Order`だけを`SavedOrder`という別名でも使います。

このディレクトリで実行します。`check`と`lower`にはNagiが必要です。`run`にはRust/CargoとNagiアプリをビルドできる環境も必要です。

```sh
nagic check
nagic lower
nagic run
```

出力は次のとおりです。

```text
42
42
7
```

`saved_total`の引数は`SavedOrder`で、`orders.total`の引数と同じ型です。`total = orders.total`ではmoduleの関数を値として使います。`archive.Order`は別の型なので、`saved_total(older)`に変更すると型エラーになります。フィールド名と構成が同じでも、別ファイルのclassを混同しません。

この例のclassは数値だけを持つため、同じ値を2回渡せます。文字列や配列を持つclassの所有権は従来のmove規則に従います。module名で公開するのは、そのファイル自身に定義した関数とclassです。読み込んだ名前は自動で再公開しません。

[English](README.en.md) · [importとRust連携](../../../docs/modules-and-rust.md)
