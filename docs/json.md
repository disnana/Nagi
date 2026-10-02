# JSONを読み書きする

JSONを読むときは、受け取りたいデータをclassで定義します。`json_decode[User](text)`でJSONから`User`を作り、`json_encode(user)`でJSONの文字列に戻せます。

```nagi
class User:
    name: str
    age: i32

def main() -> Result[unit, Error]:
    user = try json_decode[User](
        "{\"name\":\"Nagi\",\"age\":20}"
    )
    print(user.name)
    encoded = try json_encode(user)
    return ok(print(encoded))
```

`json.nagi`として保存し、`nagic run json.nagi`で実行します。`Nagi`と、名前・年齢を含むJSONを表示します。読み書きに失敗する場合があるため、`try`でエラーを呼び出し元へ返しています。詳しくは[エラー処理](error-handling.md)を参照してください。

## 入力の検査

必須フィールドの欠落、余分なフィールド、型の違い、数値の範囲外、不正なUTF-8はエラーになります。たとえば`age: i32`に文字列や範囲外の整数は渡せません。

JSONは指定した型へ直接読み込みます。文字列のフィールドはデータを所有するため、読み込み時に保存領域を確保します。classのフィールドを入力から借りる機能や、省略された値を既定値で補う設定は未対応です。

HTTPでJSONを受け取ったり返したりする例は、[HTTPとHTML](http.md)にあります。
