# 文法の早見表

[目次](README.md) · 初めて書くなら：[入門ガイド](language-guide.md) · 関数を調べる：[組み込み関数](builtins.md)

このページはHigh（`.nagi`）の書式を引くための資料です。字下げと型付きの関数・値を使ってアプリを書く構文です。短い例は関数内に書く断片も含みます。各例をまとめて動かすには[入門の完成コード](../examples/tutorial/basics.nagi)を使ってください。

Pythonでよく書く処理から探すなら、次の入口を使ってください。

| やりたいこと | Pythonでの書き方 | 現在のNagiで学ぶ場所 |
|---|---|---|
| 値を更新する | `count += 1` | [値と型](language-guide.md#1-値と型)。再代入でも型は同じ |
| 関数を使う | `def add(a, b):` | [関数](language-guide.md#2-関数を定義する)。引数型を指定し、実行はmainから |
| リストへ追加する | `items.append(value)` | [配列と反復](language-guide.md#3-配列class分岐繰り返し)。`append(items, value)` |
| 渡した値を後でも使う | `b = a`で同じ値を参照 | [viewとcopy](language-guide.md#4-読むだけならviewで借りる)、[共有](ownership.md#同じ値を複数の場所で持つ) |
| 値がない場合を分ける | `if value is None:` | [Some／None](error-handling.md#値がない場合を扱う)のmatch |
| 失敗をその場で扱う | `try/except` | [Resultのmatch](error-handling.md#成功と失敗を分ける)。`try 式`はErrの伝播 |
| 非同期処理を待つ | `await operation()` | [await](language-guide.md#7-asyncとapiへ進む)、[spawnとscope](async.md) |

## ファイルと字下げ

- UTF-8で保存し、拡張子を`.nagi`にする。
- トップレベルには`import`・`from`、`class`、`enum`、`def`、`async def`、Rustの外部関数宣言を書く。実行する文は関数内に書く。
- ブロックの前に`:`を付け、空白で字下げする。空白4つを推奨する。タブは禁止。
- コメントは`#`。識別子は英字・数字・`_`で、数字からは始めない。日本語の文字列・コメントは使える。
- `()`や`[]`の中は複数行に分けられる。引数や要素の末尾の余分な`,`は未対応。

```nagi
def main():
    # コメント
    print("Hello, Nagi!")
```

## 値と変数

| 書き方 | 意味 |
|---|---|
| `count = 10` | 型を推論する。整数の標準は`i64` |
| `count: i32 = 10` | 型を指定する |
| `rate = 1.5` | 小数の標準は`f64` |
| `enabled = True` / `False` | 真偽値。`true` / `false`も受け付ける |
| `name = "Nagi"` / `'Nagi'` | UTF-8文字列 |
| `values = [1, 2, 3]` | 同じ型の要素の配列 |
| `values: List[i64] = []` | 空配列には型注釈を付ける |
| `missing: i64? = None` | 値がないnullable。`null`も受け付ける |
| `present: i64? = some(42)` | 値があるnullable |
| `count = 11` | 同じ型で再代入 |
| `count += 1` / `-= 1` / `*= 2` | 複合代入。`/=` / `%=`は未対応 |

変数へ別の型の値を再代入することはできません。明示moveの移行では、既存の非Copyローカルの代入に`std.ownership.move`を使います。新値生成とCopy代入には不要です。実装・収録版は[代入の規則](ownership.md#代入と明示move)を参照してください。

文字列のエスケープは`\n`、`\r`、`\t`、`\"`、`\'`、`\\`です。文字列の`+`による連結、f-string、文字列の補間、三重引用符は未対応です。型の一覧は[型](types.md)を参照してください。

## 関数とreturn

呼び出し先は、同名のローカル変数に入れた関数、自作関数、組み込み関数の順に決まります。自作の`len`を定義すると、`len(...)`はその関数を呼びます。関数以外の変数は呼び出せません。

`type`などRust側の予約語も、Nagiの文法で使える位置なら名前にできます。生成Rustで必要な名前の変換はコンパイラが行い、Low・JSON・SQLiteの項目名には元の名前を残します。

```nagi
def add(a: i64, b: i64) -> i64:
    return a + b

def show(value: i64):
    print(value)
    return
```

引数の型は必須です。戻り値を省略すると`unit`。値を返す関数は全経路で戻り値が必要です。呼び出しは`add(1, 2)`のように位置引数を使います。デフォルト引数、可変長引数、ユーザー定義generic関数はありません。

## 分岐とループ

次は関数内の断片です。

```nagi
score = 80
if score >= 80:
    print("合格")
else:
    print("再挑戦")

for index in range(3):
    print(index)

count = 0
while count < 3:
    count += 1
```

条件は`bool`です。`range(n)`は`0`以上`n`未満で、引数は1つです。配列やviewの`for`は、Copy要素を値で読み、非Copy要素を読み取り専用で借ります。[所有権](ownership.md)で制約を確認できます。`elif`、`break`、`continue`、`pass`はありません。

## 演算子

優先順位が高いものから並べています。同じ段の二項演算は左から評価する形で解析します。曖昧な式には`()`を付けてください。

| 優先順位 | 演算子 | 例 |
|---|---|---|
| 高 | 呼び出し、フィールド、index | `add(1, 2)`、`point.x`、`values[0]` |
| ↓ | `-`（単項）、`not`、`try`、`await` | `-count`、`not enabled`、`try await db_open(...)` |
| ↓ | `*`、`/`、`%` | `count * 2` |
| ↓ | `+`、`-` | `count + 1` |
| ↓ | `<`、`>`、`<=`、`>=` | `count < 10` |
| ↓ | `==`、`!=` | `count == 10` |
| ↓ | `and` | `count > 0 and count < 10` |
| 低 | `or` | `enabled or count == 0` |

符号反転の`-value`は、符号付き整数（i8 / i16 / i32 / i64）と浮動小数点数（f32 / f64）に使えます。関数やclassの値には使えません。

| 比較する値 | `==` / `!=` | `<` / `>` / `<=` / `>=` |
|---|---|---|
| 同じ型の数値・bool・str | 対応 | 対応 |
| UUID・timestamp | 対応 | 未対応 |
| view[str]・view[bytes] | 対応 | 対応 |
| view[T] | 要素Tが一致比較に対応する場合 | 要素Tが大小比較に対応する場合 |
| Method・Status | 対応。Methodは借用して比較する | 未対応。Statusの数値は`.value`で比較する |
| class・所有するList | 未対応 | 未対応 |

classの配列を借りた`view[Point]`も、そのまま比較できません。必要なフィールドを取り出して比較してください。配列のviewは、要素ごとの比較になります。

異なる数値型は暗黙に変換しません。`i32`から`i64`には`i64(value)`を使います。`i32(i64値)`は`Result[i32, Error]`を返すため、Result関数内で`try i32(value)`などと書きます。

比較の連鎖`0 < count < 10`は使わず、`count > 0 and count < 10`と書きます。整数の`/`は整数除算です。`**`、`//`、ビット演算は未対応です。

## class、配列、view

```nagi
class Point:
    x: f64
    y: f64

def main():
    point = Point(x=1.0, y=2.0)
    print(point.x)
    values = [10, 20]
    append(values, 30)
    print(values[0])
    borrowed = view(values)
    duplicate = copy(borrowed)
    print(len(duplicate))
```

classは全フィールドを名前付きで指定します。フィールド・indexへの代入、method、継承は未対応です。indexは0からで、負数や範囲外は実行時panicになります。文字列のindexは使えません。文字列の`len`はUTF-8のbyte数で、Pythonの`len(str)`の文字数とは違います。`len("あ")`は`3`です。配列の`len`は要素数です。配列や文字列の区間を借りる場合は`try slice(view(data), start, end)`です。

自作関数への所有文字列・配列の引き渡しはmoveです。読むだけなら引数を`view[str]`や`view[i64]`にし、`view(value)`で渡します。詳細は[入門ガイド](language-guide.md)と[所有権](ownership.md)を参照してください。

## Result、async、scope

| 書き方 | 意味 |
|---|---|
| `-> Result[i64, Error]` | 成功なら整数、失敗ならErrorを返す |
| `return ok(42)` | 成功を返す |
| `return error("理由")` | 失敗を返す |
| `return not_found("理由")` | 対象なしを返す。HTTPでは404 |
| `return fail(problem)` | Error・独自class・enumの失敗値を返す |
| `value = try parse_i64("42")` | 値を取り出す。失敗なら呼び出し元へ返す |
| `async def work():` | 非同期関数を定義する |
| `await sleep(10)` | 非同期処理を待つ。単位はミリ秒 |
| `db = try await db_open(":memory:")` | 非同期処理を待ち、Resultの失敗も伝える |

`try`は同じエラー型のResultを返す関数内、`await`はasync関数内で使います。`try`はPythonの例外捕捉ではなく、成功値を取り出し、Errを呼び出し元へ返します。その場でResultの成功・失敗を処理する場合は、次のように両方のcaseを書きます。これは関数内の断片です。

```nagi
match parse_i64("42"):
    case Ok(number):
        print(number)
    case Err(problem):
        print(error_kind(problem))
```

値がない場合の`T?`（Option）は処理のErrやpanicとは別です。`case Some(value):`と`case None:`の両方を書いて取り出します。[不在の実行例](error-handling.md#値がない場合を扱う)も参照してください。`Some(_)`で値を捨てられます。

使わないpayloadは`_`にします。matchは対象の所有値を消費し、payloadの名前はcase内だけで使えます。外側の変数と同じ名前は使えません。`Result[T, E]`のEには独自class・enumも使えます。enumは`case Choice.Cancelled:`や`case Choice.Selected(id):`で全種類を処理します。[型の定義](types.md#enumで種類を分ける)と[エラー処理](error-handling.md)を参照してください。

子taskは次の完全なコードのようにscope内でspawnします。

```nagi
async def main() -> Result[unit, Error]:
    async with scope:
        spawn sleep(10)
        spawn sleep(15)
    return ok(print("完了"))
```

このコードは両方のsleepが終了してから`完了`を表示します。通常のscope終了では子taskを待ちます。旧statement spawnでは本体終了後に子のErrやpanicを検出すると、残りの子を止めて待ちます。親Futureの破棄や本体panicでは停止要求と終了確認を区別します。scope内の`return`とviewを別taskへ渡すことは未対応です。旧statement spawnはunitか`Result[unit, Error]`を返すasync呼出しに限ります。[Task結果handle](task-handles.md)はNagi 0.1.11で公開済みです。Task bindingはawait時に子の結果をscope body内で受け取ります。受取failureを処理してもscope faultは残り、scope出口では子の実joinを待ちます。利用するcompilerが0.1.11以降であることを確認してください。詳細は[async](async.md)を参照してください。

## import、HTTP、Rust

| 用途 | 書き方 | 詳細 |
|---|---|---|
| ファイルを読み込む | `import "models.nagi"` | [import](modules-and-rust.md)。同じ名前空間に読み込む |
| module名を付ける | `import "orders.nagi" as orders` | `orders.Order`や`orders.score(...)`で、そのファイル自身の定義を使う |
| 定義を選ぶ | `from "orders.nagi" import Order as SavedOrder` | `,`で複数の定義を選べる。各`as 名前`は省略できる |
| 標準HTTPを読み込む | `import std.http.server as http` | `http.Request`や`http.Status.OK`を使う。`as`は必須 |
| 標準型を選ぶ | `from std.http.server import Request, Response, Status as Code` | [標準import](modules-and-rust.md#標準http-libraryを読み込む) |
| GET handlerを定義する | 関数の前に`@get("/users/{id}")` | [HTTP](http.md)。`@post`、`@put`、`@delete`もある |
| HTTP公開方針 | `http.public_policy[State]()` | 全routeへ明示Policy。active HTMLはSF04まで未対応 |
| テキストを埋め込む | `include_text("index.html")` | ソースの場所を基準に、コンパイル時に埋め込む |
| Rust関数を宣言する | `@rust("native::crc32")`の次行に`extern def crc32(text: view[str]) -> i64` | [Rust連携](modules-and-rust.md)。本体・末尾の`:`は不要 |

`orders.Order`と`SavedOrder`は同じ型です。別ファイルの同名classは別の型として扱います。`from`・`as`はimport文だけのキーワードで、既存の識別子としても使えます。

## Pythonに似ていても違うところ

| Pythonでよく書く形 | Nagiでは |
|---|---|
| `def add(a, b):` | 引数型を書く。戻り値があるなら`-> 型`も書く |
| `print(a, b)` | 1引数ずつ`print(a)`、`print(b)` |
| `items.append(x)` | `append(items, x)` |
| `try: ... except:` | `try 式`で失敗を伝える、または`match`でResultを分岐する |
| `from models import User` | `from "models.nagi" import User`。ファイルの相対パスを引用符で囲む |
| 辞書、tuple、内包表記、lambda | 未対応。class、配列、通常の関数・ループを使う |
| `str(42)`、任意型へのcast | 汎用変換は未対応。直接`print(42)`などを使う |

nullableは`None` / `some(value)`で作り、`case Some(value):` / `case None:`のmatchで取り出します。汎用unwrap APIはありません。

整数のrelease演算はRust backendの固定幅演算に従い、加算などのoverflowはwrapします。debug Rust側ではpanicする場合があります。checked / wrapping演算を言語として統一することは今後の課題です。Lowの構文・差し替えは[Low](low-language.md)、実装予定は[roadmap](roadmap.md)にあります。
