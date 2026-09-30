# viewとゼロコピー

viewは既存メモリを借用します。`view(str)`、`view(List)`と、`slice(view, start, end)`で元のメモリを使用します。sliceの範囲やUTF-8境界の誤りはResultで返します。

HTTP handlerの`body: view[bytes]`は、Axumが保持するBytesからsliceを借ります。この借用に追加のbodyコピーはありません。ただし、OSからHTTP bufferへの受信やbody frameのcollectまでが無コピーになるという意味ではありません。

実証では、元bufferとsliceのpointer差が指定offsetと一致し、allocationが0であることを確認します。借用JSONの&strにも入力buffer内のaddressであることを確認します。

JSONのescapeを解く文字列は、そのままの入力byte列とは一致しません。所有Stringまたはcopyが必要です。SQLiteのTEXT/BLOBも次のstepで無効になり得るため、workerを越えて返す値は所有化します。

```python
def identity(data: view[str]) -> view[str]:
    return data
```

入力viewに由来するviewは返せます。ローカルString、所有引数、requestの寿命を越えて参照しようとするコードは拒否します。0.1はclassのborrowed fieldと、別taskへのrequest viewの貸し出しをサポートしていません。
