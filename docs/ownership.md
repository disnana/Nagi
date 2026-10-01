# 所有権

str、bytes、List、所有fieldを持つclassは、関数へ渡すとmoveします。move後の名前の使用はcheckerで拒否します。Copy値はそのまま複数回使用できます。

```python
def use_name(name: str):
    print(name)

def main():
    name = "alice"
    use_name(name)
    # print(name) はmove後の使用
```

## classのフィールドを取り出す

文字列のフィールドを別の変数へ代入すると、その文字列の所有権が移ります。移動したフィールドは再利用できませんが、移動していない別のフィールドは使えます。

```nagi
class Person:
    name: str
    age: i64

def main():
    person = Person(name="Nagi", age=1)
    name = person.name
    print(name)
    print(person.age)
    # print(person.name) はmove後の使用
```

元のフィールドも残したい場合は、上の代入を`name = copy(view(person.name))`に変更します。コピーはmoveする前に作ってください。`print`や`len`で読み取るだけなら、文字列はmoveしません。

関数に読み取り用の値を渡す例は、[読むだけならviewで借りる](language-guide.md#4-読むだけならviewで借りる)にあります。

## ループで同じ値を使う

ループの外で作った文字列を、毎回所有値として渡すと、1周目でmoveして次の周回で使えなくなります。`check`はこの再利用を拒否します。フィールドを取り出す場合や、`while`の条件で値を渡す場合も同じです。

元の値を残して渡すには、周回ごとにコピーを作ります。読むだけの関数なら、引数を`view[str]`にして`view(name)`を渡せます。

```nagi
def use_name(name: str):
    print(name)

def main():
    name = "Nagi"
    for number in range(2):
        use_name(copy(view(name)))
    print(name)
```

次の周回までに新しい値を代入する方法もあります。

```nagi
def main():
    name = "first"
    for number in range(2):
        use_name(name)
        name = "next"
    print(name)
```

分岐で再代入する場合は、次の周回へ進むすべての経路で値を用意してください。`return`で関数を終える経路は、次の周回や後続の処理へ影響しません。ループ内で新しく作った値は周回ごとに使えます。

検査は実行回数を計算せず、繰り返しと0回の実行を考慮します。そのため、ループ内の代入だけでは、ループ後に値が使えるとは限りません。`while`の条件は、ループを抜けるときにも評価されます。

## 借用と検査の範囲

viewを作った所有値をmove・再代入・appendすることも制限します。viewの借用は字句scopeで保守的に追跡します。Rustのnon-lexical lifetimeと同等の精密な解析はまだありません。

部分fieldのmove、複雑な分岐・ループ、genericな借用のsoundnessをNagi checkerだけで証明していません。Rust codegenは安全なRustを出し、backendの借用検査にも通ったものだけを実行ファイルにします。`check`の成功と`build`の成功を区別してください。

所有権を推論する狙いは、アプリ開発者にlifetime記法を増やさず、コピーが必要な場面を示すことです。所有データをもう一つ保持する場合は`copy(view(data))`を記述します。
