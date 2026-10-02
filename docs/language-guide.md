# コードを書きながら学ぶ

[目次](README.md) · 前：[準備と最初の実行](getting-started.md) · 調べる：[文法の早見表](syntax.md)

アプリを書くHigh（`.nagi`）を、変数 → 関数 → 配列 → class → 失敗の扱いの順に学びます。[準備と最初の実行](getting-started.md)でNagiをインストールし、自分の作業フォルダーにコードを保存してください。Windows・Linux・macOSで同じ`nagic`コマンドを使います。

## 1. 値と型

次は`main`などの関数の中に書く断片です。

```nagi
count = 10            # i64。型は自動で決まる
rate = 1.5            # f64
enabled = True       # bool。True / Falseと書く
name = "Nagi"        # str。日本語も書ける
age: i32 = 18        # 型を指定するなら「名前: 型 = 値」
count += 1
print(count)
```

変数は再代入できますが、型を変更することはできません。`count = "十"`は型エラーです。条件には`bool`を使い、`if count:`のように整数を真偽値として扱うことはできません。

異なる整数型は自動で混ざりません。`i64(age)`で`i32`を`i64`へ広げられます。逆の`i32(count)`は範囲外の可能性があるので`Result[i32, Error]`を返します。失敗の扱いは後で説明します。

## 2. 関数を定義する

```nagi
def add(a: i64, b: i64) -> i64:
    return a + b

def main():
    print(add(20, 22))
```

この完全なコードを保存して実行すると`42`と表示します。引数は`名前: 型`、戻り値は`-> 型`です。戻り値を省略した関数は`unit`（値を返さない）です。`main`がプログラムの入口になります。

## 3. 配列、class、分岐、繰り返し

配列は`[1, 2, 3]`、型注釈は`List[i64]`です。`append(values, 4)`で末尾に追加します。`class`は名前付きフィールドをまとめる型です。

次の完全なコードを`basics.nagi`として保存してください。[サンプル](../examples/tutorial/basics.nagi)にも同じコードがあります。

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
- `for value in values`は要素を順に読みます。現在は整数・小数・boolなどの基本の値（primitive）や、それだけを含むclassを走査できます。このように所有権を移さずコピーできる型をCopyと呼びます。`List[str]`の走査は未対応です。
- `range(3)`は`0, 1, 2`です。引数は1つで、終端を含みません。
- `while`は条件が`True`の間繰り返します。`break` / `continue`は未対応です。
- `and` / `or` / `not`で条件を組み合わせます。`elif`はないため、必要なら`else`の中に`if`を書きます。

`view(values)`は「配列を渡して手放す」代わりに「読み取り用に借りる」書き方です。次で説明します。

## 4. 読むだけならviewで借りる

文字列や配列を自作関数へそのまま渡すと、所有権が移動します。受け渡しを**move**と呼びます。渡した変数を後で使うとエラーになります。

```nagi
# エラーになる例
def take_name(name: str):
    print(name)

def main():
    name = "Nagi"
    take_name(name)
    print(name)       # 自作関数へmoveした後なので使えない
```

読むだけの関数は`view[str]`を受け取るようにします。呼び出すときは`view(name)`です。次は完全なコードで、[borrowing.nagi](../examples/tutorial/borrowing.nagi)として実行できます。

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

出力は`4`、`4`、`Nagi`、`Nagi`です。`copy(view(name))`は別の所有文字列を作ります。コピーが必要なときだけ明示してください。

`print`や`len`は入力を読み取る組み込み関数なので、それらに渡しただけでは文字列をmoveしません。すべての関数が同じ受け渡し方をするわけではありません。

`len(str)`は文字数ではなくUTF-8のbyte数です。`len("あ")`は`3`です。viewが生きている間の元データの変更にも制約があります。詳細は[所有権](ownership.md)にあります。

## 5. 失敗する処理はResultで返す

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

`Ok`と`Err`を1回ずつ書きます。括弧内の名前はそのcase内で使い、不要な値は`_`にします。`Result`以外のパターンは未対応です。実行できる完成コードとErrorの調べ方は[エラー処理](error-handling.md)にあります。

## 6. ファイルを分ける

同じディレクトリに2つのファイルを作ります。

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

現在は全ファイルが1つの名前空間に入ります。`math.add(...)`やaliasではなく`add(...)`で呼び、関数名・class名の重複はエラーです。Rustライブラリを使う場合は[importとRust連携](modules-and-rust.md)へ進んでください。

## 7. asyncとAPIへ進む

タイマーやI/Oを待つ関数は`async def`で定義し、呼び出す側で`await`します。

```nagi
async def main() -> Result[unit, Error]:
    await sleep(10)
    print("10ミリ秒待った")
    return ok(print("完了"))
```

これは完全なコードです。`sleep`は`unit`を返すので`await`だけ、失敗し得るDB関数などは`try await db_open(...)`のように書きます。`await`は非同期処理を待ち、`try`は待った結果の失敗を伝えます。

次は[HTTPとHTML](http.md)で、ブラウザーから呼べるAPIを作ってください。書式だけ調べたいときは[文法の早見表](syntax.md)、関数の引数を調べたいときは[よく使う関数](builtins.md)を使えます。
