# viewでコピーせずに読む

`view`は、文字列や配列の中身を借りて読むための値です。元のデータをコピーせず、所有権も移しません。元のデータが使える間だけ利用できます。

```nagi
def main() -> Result[unit, Error]:
    text = "Nagi language"
    borrowed = view(text)
    first = try slice(borrowed, 0, 4)
    print(first)
    saved = copy(first)
    print(saved)
    return ok(print(text))
```

`view.nagi`として保存し、`nagic run view.nagi`で実行すると、`Nagi`、`Nagi`、`Nagi language`を表示します。`first`は元の文字列の一部を借りています。`saved`は`copy`で作った別の所有文字列です。

## 一部を借りる

`slice(view(data), start, end)`は、`start`以上`end`未満の範囲を借ります。文字列では位置をUTF-8のバイト数で指定します。範囲外や文字の途中を指定すると、`Result`のエラーになります。

保存するviewは、上の例のように、先に元の値を変数へ入れてから作ってください。借りた値を使っている間は、元の値の変更や移動にも制約があります。[所有権](ownership.md)に具体例があります。

## 関数から返す

入力のviewをそのまま返したり、その一部を返したりできます。

```nagi
def identity(data: view[str]) -> view[str]:
    return data
```

関数の中で作った文字列を借りて返すことはできません。関数が終わると元のデータがなくなるためです。長く保持したいデータは所有値として返します。classへのviewの保存や、別のtaskへviewを渡すことは未対応です。

HTTPの`body: view[bytes]`も受信データを借ります。viewを作るための追加コピーはありませんが、ネットワークからの受信やJSONの読み込みには別の処理が必要です。測定方法は[性能の読み方](performance.md)を参照してください。
