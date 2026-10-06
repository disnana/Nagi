# コードを書きながら学ぶ

[目次](README.md) · 前：[準備と最初の実行](getting-started.md) · 調べる：[文法の早見表](syntax.md)

Pythonで関数やリストを書いたことがある人向けに、同じ目的を現在のNagiでどう書くかを試します。High（`.nagi`）は字下げで書く構文ですが、値を渡すときや失敗を扱うときにはPythonとの違いがあります。[Nagiをインストール](getting-started.md)し、作業用のフォルダーで`nagic run ファイル名.nagi`を実行してください。

## 1. 値と型

数を増やして表示したいとき、Pythonでは次のように書けます。

```python
count = 10
count += 1
print(count)
```

Nagiでも代入から始められます。次の断片は`main`などの関数の中に書きます。

```nagi
count = 10            # i64。型は自動で決まる
rate = 1.5            # f64
enabled = True       # bool。True / Falseと書く
name = "Nagi"        # str。日本語も書ける
age: i32 = 18        # 型を指定するなら「名前: 型 = 値」
count += 1
print(count)
```

この断片は`11`を表示します。変数は再代入できますが、型を変更することはできません。`count = "十"`は型エラーです。条件には`bool`を使い、`if count:`のように整数を真偽値として扱うことはできません。

異なる整数型は自動で混ざりません。`i64(age)`で`i32`を`i64`へ広げられます。逆の`i32(count)`は範囲外の可能性があるので`Result[i32, Error]`を返します。失敗の扱いは後で説明します。

一言でいうと、代入から型が決まり、同じ型で更新します。[型の一覧](types.md)と[変数の書式](syntax.md#値と変数)で詳しく調べられます。

## 2. 関数を定義する

足し算を何度も使いたいなら、関数にします。Pythonでは引数の型を省略できます。

```python
def add(a, b):
    return a + b

print(add(20, 22))
```

Nagiでは引数と、返す値の型を書き、実行する処理を`main`に置きます。

```nagi
def add(a: i64, b: i64) -> i64:
    return a + b

def main():
    print(add(20, 22))
```

このコードを保存して実行すると`42`と表示します。引数は`名前: 型`、戻り値は`-> 型`です。戻り値を省略した関数は`unit`（値を返さない）です。`main`がプログラムの入口になります。

Pythonのようにファイル末尾へ`print(add(...))`を直接書くと、Nagiでは受理されません。上の例のように`main`へ入れます。一言でいうと、型付きの関数を定義し、`main`から呼びます。[関数とreturn](syntax.md#関数とreturn)で制約を確認できます。

## 3. 配列、class、分岐、繰り返し

いくつかの数をまとめて合計し、結果で処理を分けたいとします。Pythonのリスト追加や反復は次のように書けます。

```python
values = [1, 2, 3]
values.append(4)
for value in values:
    print(value)
```

Nagiの配列は`[1, 2, 3]`、型注釈は`List[i64]`です。`append(values, 4)`で末尾に追加します。`class`は名前付きフィールドをまとめる型です。

次のコードを`basics.nagi`として保存してください。[サンプル](../examples/tutorial/basics.nagi)にも同じコードがあります。

```nagi
class Point:
    x: f64
    y: f64

def sum_numbers(values: view[i64]) -> i64:
    total = 0
    for value in values:
        total += value
    return total

def main():
    values = [1, 2, 3]
    append(values, 4)
    total = sum_numbers(view(values))
    print(total)

    point = Point(x=3.0, y=4.0)
    print(point.x + point.y)

    if total >= 10 and len(values) == 4:
        print("OK")
    else:
        print("NG")

    for index in range(3):
        print(index)

    count = 0
    while count < 2:
        count += 1
    print(count)
```

```powershell
nagic run basics.nagi
```

プログラムの出力は順に`10`、`7`、`OK`、`0`、`1`、`2`、`2`です。

- `Point(x=..., y=...)`は全フィールドを名前付きで指定します。`Point(3.0, 4.0)`とは書きません。
- `point.x`でフィールドを読みます。現在はclassにmethodや継承を定義できません。
- `for value in values`は要素を順に読みます。数値などの要素は値をコピーし、文字列やそれを持つclassは読み取り専用で借ります。暗黙にコピーできる性質をCopyと呼びます。借用中は元の配列を変更できません。[所有権](ownership.md)に詳しい例があります。
- `range(3)`は`0, 1, 2`です。引数は1つで、終端を含みません。
- `while`は条件が`True`の間繰り返します。`break` / `continue`は未対応です。
- `and` / `or` / `not`で条件を組み合わせます。`elif`はないため、必要なら`else`の中に`if`を書きます。

`values.append(4)`はNagiでは使えません。`append(values, 4)`に直します。一言でいうと、配列を繰り返し読み、classで名前付きの値をまとめます。[分岐とループ](syntax.md#分岐とループ)と[型](types.md)で詳しく調べられます。

`view(values)`は「配列を渡して手放す」代わりに「読み取り用に借りる」書き方です。次で説明します。

## 4. 読むだけならviewで借りる

関数に文字列を読ませた後も、手元で使いたいとします。Pythonでは代入しただけで新しいリストは作られず、二つの名前が同じリストを指します。

```python
a = [1, 2]
b = a
b.append(3)
print(a)  # [1, 2, 3]
```

Nagiでは、読むだけの関数は`view[str]`を受け取り、呼び出す側は`view(name)`で貸します。次は完全なコードで、[borrowing.nagi](../examples/tutorial/borrowing.nagi)として実行できます。

```nagi
def name_size(name: view[str]) -> i64:
    return len(name)

def main():
    name = "Nagi"
    print(name_size(view(name)))
    print(name_size(view(name)))
    duplicate = copy(view(name))
    print(duplicate)
    print(name)
```

出力は`4`、`4`、`Nagi`、`Nagi`です。`copy(view(name))`は別の所有文字列を作ります。元の文字列とは別の領域を使います。

文字列や配列を自作関数へ所有値として渡すと、その値を手放します。この受け渡しを**move**と呼びます。

別のローカル変数へ文字列を渡す場合は、次のように明示します。この代入規則は作業branchで実装済み・未リリースです。

```nagi
from std.ownership import move

def main():
    name = "Nagi"
    destination = move(name)
    print(destination)
```

出力は`Nagi`です。`destination = name`では所有する非Copyローカルの通常代入として拒否され、`move(name)`で渡した後の`name`も使えません。元の文字列も読むなら、moveした後にcopyを追加するのではなく、代入を`destination = copy(view(name))`へ置き換えます。数値等のCopy値と新しい値を作る式は通常代入でき、関数への引数・return・field/indexの既存ルールは変わりません。importの別名と正確な範囲は[代入と明示move](ownership.md#代入と明示move)で確認できます。

次は意図的に`check`エラーになる完全な例です。

```nagi
# エラーになる例
def take_name(name: str):
    print(name)

def main():
    name = "Nagi"
    take_name(name)
    print(name)       # 自作関数へmoveした後なので使えない
```

渡した`name`は後で使えません。読むだけなら、最初の例のように引数を`view[str]`へ変えて`view(name)`を渡します。一言でいうと、読むだけならviewで貸します。

`print`や`len`は入力を読み取る組み込み関数なので、それらに渡しただけでは文字列をmoveしません。すべての関数が同じ受け渡し方をするわけではありません。

`len(str)`は文字数ではなくUTF-8のbyte数です。`len("あ")`は`3`です。viewが生きている間の元データの変更にも制約があります。詳細は[所有権](ownership.md)にあります。

## 5. 失敗する処理はResultで返す

入力を数に変えたいとき、Pythonでは`int(text)`が失敗すると例外を送出します。呼び出し元へ伝えるか、その場で扱うかを選べます。

```python
try:
    number = int("abc")
except ValueError:
    number = 0
```

Nagiでは処理の失敗を返却値として扱います。値がないだけなら`T?`、処理の成功か失敗を返すなら`Result`、通常の入力不正などを返す用途ではない異常はpanicと区別します。`T?`の取り出し方は[値がない場合を扱う](error-handling.md#値がない場合を扱う)で試せます。

`Result[T, Error]`は「成功なら`T`、失敗なら`Error`」という戻り値です。`ok(value)`で成功、`error("理由")`で失敗を返します。

`try 処理`は、成功した値を取り出し、失敗したらその場で呼び出し元へ失敗を返します。そのため、`try`を書く関数自身も`Result`を返す必要があります。

次は入力した整数を2倍にする完全なコードです。`input.nagi`として保存してください。[サンプル](../examples/tutorial/input.nagi)にもあります。

```nagi
def double_nonnegative(text: view[str]) -> Result[i64, Error]:
    value = try parse_i64(text)
    if value < 0:
        return error("0以上の整数を入力してください")
    return ok(value * 2)

def main() -> Result[unit, Error]:
    write("整数を入力 > ")
    text = try read_line()
    doubled = try double_nonnegative(view(text))
    print(doubled)
    return ok(print("完了"))
```

```powershell
nagic run input.nagi
```

`21`を入力してEnterを押すと、`42`と`完了`を表示します。`abc`は数値の変換に失敗し、`-1`は自分で書いたエラーになります。どちらも`main`まで伝わり、プログラムは失敗で終了します。

Pythonのintと異なり、i64には上限があります。この例は小さな整数を想定し、2倍にする前の範囲確認は省いています。大きな入力では[整数演算の規則](syntax.md#演算子)も確認してください。

`write`は改行なし、`read_line`は1行入力です。`return ok(print("完了"))`は、表示処理が返す`unit`を成功として返しています。

Nagiの`try`はPythonの`try: ... except:`とは別の構文です。その場で既定値に回復するなど、成功・失敗を分けたいときは`match`を使います。次は関数の例です。

```nagi
def number_or(text: view[str], fallback: i64) -> i64:
    match parse_i64(text):
        case Ok(number):
            return number
        case Err(_):
            return fallback
```

この関数は`"21"`なら`21`、`"abc"`なら指定したfallbackを返します。Resultでは`Ok`と`Err`の両方を書きます。括弧内の名前はそのcase内で使い、不要な値は`_`にします。`match`はnullableの`Some`／`None`と独自enumにも対応します。`parse_i64(text)`を呼ぶだけでResultを捨てると`check`が拒否します。伝えるなら`try`、回復するなら上の`match`で処理します。一言でいうと、失敗も返却値として受け取って扱います。詳しくは[エラー処理](error-handling.md)と[型](types.md)を参照してください。

## 6. ファイルを分ける

関数を別ファイルへ分けたいとき、Pythonなら`from math_helpers import add`のようにmodule名で読み込みます。Nagiではファイルの相対パスを引用符で囲みます。同じディレクトリに2つのファイルを作ります。

`math.nagi`：

```nagi
def add(a: i64, b: i64) -> i64:
    return a + b
```

`imports.nagi`（実行するファイル）：

```nagi
import "math.nagi"

def main():
    print(add(20, 22))
```

```powershell
nagic run imports.nagi
```

[完成ファイル](../examples/tutorial/imports.nagi)を実行すると`42`です。importのパスは、実行したターミナルではなく**importを書いたファイルの場所**を基準にします。読み込まれる`math.nagi`には`main`は不要です。

この書き方では同じ名前空間へ読み込むため、`add(...)`で呼びます。`import "math.nagi" as math`に変えると`math.add(...)`で呼べます。classや関数だけを選ぶfrom文も使えます。`import math`だけではこのファイルを読み込めません。`import "math.nagi"`に直します。一言でいうと、import元からの相対パスで読み込みます。同名の定義を区別する方法やRustライブラリとの連携は[importとRust連携](modules-and-rust.md)を参照してください。

## 7. asyncとAPIへ進む

タイマーやI/Oの結果を待ちたいとき、Pythonでも`async def`の中で`await`します。

```python
import asyncio

async def wait_once():
    await asyncio.sleep(0.01)  # 秒
```

Nagiも`async def`と`await`を使います。次は完全なコードです。

```nagi
async def main() -> Result[unit, Error]:
    await sleep(10)
    print("10ミリ秒待った")
    return ok(print("完了"))
```

`10ミリ秒待った`、`完了`の順で表示します。Pythonの`asyncio.sleep`は秒、Nagiの`sleep`はミリ秒です。`sleep`は`unit`を返すので`await`だけ、失敗し得るDB関数などは`try await db_open(...)`のように書きます。`await`は非同期処理を待ち、`try`は待った結果の失敗を伝えます。

`sleep(10)`だけでは待っていないため`check`が拒否します。`await sleep(10)`に直します。awaitしただけで別taskが作られるわけではありません。一言でいうと、awaitで結果を待ちます。複数の処理を進める`spawn`と、その寿命を管理するscopeは[async](async.md)で試せます。

次は[HTTPとHTML](http.md)で、ブラウザーから呼べるAPIを作ってください。書式だけ調べたいときは[文法の早見表](syntax.md)、関数の引数を調べたいときは[組み込み関数](builtins.md)を使えます。
