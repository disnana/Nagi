# JSONと型付きclass

`json_decode[User](input)`はSerdeの型付きdeserializeを使います。全体を汎用Valueへ読み、dictへ直し、modelへ作り直す経路を挟みません。fieldの欠落、型不一致、i32範囲外、不明field、invalid UTF-8を拒否します。

```python
def read_user(text: str) -> Result[User, Error]:
    return json_decode[User](text)
```

所有String fieldにはallocationとコピーがあります。class本体とprimitive fieldはnative値です。入力に借用できる&strを使う実験では0 allocationを確認していますが、Highのborrowed class fieldはまだ使えません。

`json_encode`はclassから所有Stringへencodeします。HTTP responseは同じtyped serializationをVec<u8>へ行います。ランタイムの比較試験では、warm bufferにto_writerでencodeした場合のallocationも測ります。このbuffer再利用はHTTP経路へ自動適用していません。

浮動小数のNaN/Infinityをどう扱うか、未知fieldの許容モード、nullableとdefault値、response literalの直接encodeは仕様の追加対象です。
