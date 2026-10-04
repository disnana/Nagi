# JSONを読み書きする

NagiはJSONとNagiの型の対応を検査し、読み書きはSerde／serde_jsonで行います。JSONを読むときは、受け取りたいデータをclassで定義します。`json_decode[User](text)`でJSONから`User`を作り、`json_encode(user)`でJSONの文字列に戻せます。

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

## 型と借用

classのほか、数値・bool・str・Listなど、JSONに対応する型を指定できます。関数、Error、enum、Db、Htmlは読み書きの対象にできません。これらをclassやList、Resultの中に入れた場合も、`check`でエラーになります。

`json_decode[view[str]](text)`はJSONの文字列を入力から借ります。結果を保存する場合は、入力も変数に保存し、借りている間は変更・移動しないでください。独立した文字列が必要なら`json_decode[str](text)`を使います。

借用で読めるのは、入力をそのまま参照できる文字列です。JSON内の`\n`や`\uXXXX`など、エスケープの展開が必要な文字列は借用では読めず、Resultのエラーになります。その場合も`str`で読みます。

`bytes`は`[97,98,99]`のような整数配列として読み書きします。一方、`json_decode[view[bytes]]`はエスケープのないJSON文字列（例: `"abc"`）からUTF-8バイトを借り、整数配列は読めません。`json_encode`は借用したbytesも整数配列へ書くため、同じ形式で読み書きするなら`bytes`を使ってください。

## 入力の検査

必須フィールドの欠落、余分なフィールド、型の違い、数値の範囲外、不正なUTF-8はエラーになります。たとえば`age: i32`に文字列や範囲外の整数は渡せません。

JSONは指定した型へ直接読み込みます。文字列のフィールドはデータを所有するため、読み込み時に保存領域を確保します。classのフィールドを入力から借りる機能や、省略された値を既定値で補う設定は未対応です。

現在、`json_encode`はNaN・正負の無限大をエラーにせず、`null`として出力します。Listやclassの中でも同じです。`null`は`f64`へ読み戻せず、非有限値が入った`f64?`は読み戻すと`None`に変わります。

HTTPでJSONを受け取ったり返したりする例は、[HTTPとHTML](http.md)にあります。

実装は[JSON runtime](../runtime/src/lib.rs)と[classの変換生成](../compiler/src/emit.rs)にあります。[型のテスト](../compiler/tests/builtin_type_contracts.rs)では対応する型と借用の制限、[runtimeのテスト](../runtime/src/lib.rs)では不正入力とフィールドの検査を確認しています。
