# 型と推論

[目次](README.md) · 初めて読むなら：[入門ガイド](language-guide.md)

変数は`count: i32 = 10`、関数の引数は`count: i32`、戻り値は`-> i32`と書きます。ローカル変数の型は省略できますが、決まった型を再代入で変更することはできません。

| 型 | 用途 | 制約・値の扱い |
|---|---|---|
| `i8` / `i16` / `i32` / `i64` | 符号付き整数。数字はビット数 | コピーできる。異なる数値型の演算には明示的な変換が必要 |
| `u8` / `u16` / `u32` / `u64` | 符号なし整数 | コピーできる。負数を格納できない |
| `f32` / `f64` | 小数を扱う浮動小数点数 | コピーできる |
| `bool` | `True` / `False` | 分岐・ループの条件に使う |
| `str` / `bytes` | UTF-8文字列 / バイト列 | 所有値。自作関数へ渡すとmoveする |
| `List[T]` / `[T]` | 同じ型の要素を持つ配列 | 所有値。forはCopy要素をコピーし、非Copy要素を読み取り専用で借りる |
| `view[str]` / `view[bytes]` / `view[T]` | 文字列・バイト列・配列を借りて読む | 元データの所有者が必要。読むだけで元の値は変更できない |
| `T?` / `Option[T]` | 値がある、または`None` | `None`には型の文脈が必要。`Some` / `None`のmatchで取り出す |
| `Result[T, E]` | 成功値またはエラー | `E`はError・独自class・enum。`try`で伝播、`match`で処理する |
| `shared[T]` | 複数の場所で共有する所有値 | `share`で作り、`clone_shared`で共有参照を増やす |
| `UUID` / `timestamp` | UUID / 時刻の値 | UUIDの文字列変換には`uuid_parse` / `uuid_format`を使う |
| `fn[引数の型..., 戻り値の型]` | 関数を値として渡す | 同期関数の引数・戻り値に使える。最後の型が戻り値 |
| `unit` | 戻り値がないことを表す型 | 関数の戻り値型を省略したときの型 |

`count = 10`はi64、`rate = 1.5`はf64です。型注釈がある整数literalはその範囲を確認します。現在の数値変換APIは、i8 / i16 / i32 / u8 / u16 / u32からの損失のない`i64(value)`と、i64から範囲を検査する`i32(value) -> Result[i32, Error]`です。任意型への汎用castはありません。

小数リテラルも、型注釈・引数・戻り値などで決まる`f32`または`f64`の範囲を検査します。その型で無限大になる値は、型検査でエラーになります。

VS Code拡張では、変数名にマウスを置くと推論された型を確認できます。たとえば`count`は`count: i64`です。関数の引数やcaseの束縛名も対象です。[エディターの操作例](editor.md)で試せます。

nullableは`missing: i64? = None`、値がある場合は`present: i64? = some(42)`です。`T?`は`Option[T]`の短い表記です。値を取り出すには両方のcaseを書きます。

```nagi
match present:
    case Some(value):
        print(value)
    case None:
        print("値なし")
```

使わない値は`Some(_)`にします。汎用unwrap APIはありません。空配列は`values: List[i64] = []`と型を指定してください。

関数の引数・戻り値・借用の書き方は[文法](syntax.md)、コピーとmoveの規則は[所有権](ownership.md)、各関数の対応する型は[組み込み関数](builtins.md)を参照してください。

## enumで種類を分ける

```nagi
enum Choice:
    Cancelled
    Selected(id: i64)
```

`Choice.Cancelled`か`Choice.Selected(id=42)`を作り、`match`で全種類を処理します。情報を持つ種類は位置引数でも作れます。全payloadがコピー可能ならenumもコピーできます。str・Error・Listなどを含む場合はmoveします。

classと同じく、ファイルをimportして使えます。enumの型引数・メソッド・JSON変換は未対応です。[エラー処理](error-handling.md#独自のエラー型)にResultと組み合わせる例があります。

## 標準HTTPのresource型

`std.http.server`から`Request`、`Response`、`Method`、`Status`、`Options`、`App[State, E]`をimportできます。通常のclassと異なり、これらはlibraryが作るnative値です。名前付きfieldで直接構築したり、JSONへ変換したりはできません。コピーできるのは`Status`だけです。`Method`の一致比較は借用し、Requestのmethodを取り出す代入はmoveします。

`view[Request]`など、登録済みHTTP resourceのviewは値そのものへの参照です。通常の`view[Point]`は引き続きPointの配列のsliceで、Point1個への参照ではありません。Requestのbody・path・headerのviewは所有元を借り、所有元より長く保存できません。

resourceのviewには`len`・`slice`・index取得・`for`を使えません。`List[Status]`は直接index取得や反復ができますが、resourceのListからviewを作る操作は未対応です。

AppのhandlerはRequestをmoveで受け取り、stateは`shared[State]`で読みます。共有stateのCopy fieldは読み出せますが、strなどの非Copy fieldはmoveできません。`view(state.label)`で借用してください。StateのfieldにはDbを含められますが、DbやHTTP resourceを含むclassはJSON変換に対応しません。AppのState・E型引数にはviewを保持できません。詳しくは[HTTP](http.md)と[標準import](modules-and-rust.md#標準http-libraryを読み込む)を参照してください。

## 関数を値として渡す

関数名を変数へ代入したり、別の関数へ渡したりできます。`fn[i64, i64]`は「i64を1つ受け取り、i64を返す関数」です。引数がない場合は、戻り値の型だけを書きます。たとえば`fn[i64]`です。

次を`app.nagi`に保存してください。

```nagi
def add_one(value: i64) -> i64:
    return value + 1

def apply(callback: fn[i64, i64], value: i64) -> i64:
    return callback(value)

def main():
    chosen: fn[i64, i64] = add_one
    print(apply(chosen, 41))
```

```sh
nagic run app.nagi
```

結果は`42`です。`chosen = add_one`のようにローカル変数の型を省略しても推論されます。関数を返す場合は、たとえば`def choose() -> fn[i64]:`と宣言します。

async関数も`selected = answer`のように代入して、async関数内で`await selected(...)`と呼べます。HTTPの`route` / `route_mapped`はこの名前付きasync関数やローカルaliasを登録できます。一般のasync関数を受け取る引数や返す関数の型注釈にはまだ対応していません。ラムダ式や、周囲のローカル変数を取り込むクロージャも未対応です。

async関数を配列やclassへ保存することも未対応です。非同期処理を呼び出した戻り値をいったん変数へ保存する形式も使えません。`pending = sleep(10)`ではなく、`await sleep(10)`と呼び出してください。

## 未対応の型操作

`Map[K, V]`と`owned[T]`は型表記のみで、操作用のAPIは揃っていません。利用者が型引数を持つ関数を定義する機能や、traitは未対応です。

## Rustでの表現

数値とboolはRustの同じ型、`str`は`String`、`bytes`と`List`は`Vec`、`shared`は`Arc`へ変換します。`view`は参照、nullableは`Option`、成功・失敗は`Result`、enumはRustのenumです。詳しくは[メモリの扱い](memory-model.md)を参照してください。

## 符号付き整数の最小値

`i8`の`-128`、`i16`の`-32768`、`i32`の`-2147483648`、`i64`の`-9223372036854775808`もリテラルとして直接書けます。たとえば`minimum: i8 = -128`です。指定した型の範囲を超えるリテラルは、型検査でエラーになります。
