# 所有権

文字列や配列を自作関数へ渡すと、そのデータの所有権が関数へ移ります。これをmoveと呼びます。渡した変数をもう一度使うとエラーになります。数値などのコピーできる値は、そのまま繰り返し使えます。

```nagi
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
def use_name(name: str):
    print(name)

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

`for value in values`は、走査する配列を借用します。次の周回へ進む経路では、その配列の`append`・再代入・moveを`check`で拒否します。`view(values)`や、そのviewを変数に入れて走査する場合も同じです。別の配列は変更でき、ループが終了すれば走査の借用は終わります。

```nagi
def main():
    values = [1, 2]
    output: List[i64] = []
    for value in values:
        append(output, value * 2)
    append(values, 3)
```

数値などのCopy要素は、ループ変数へ値をコピーします。文字列を持つclassなど、非Copy要素は読み取り専用で借用します。反復のために文字列やclassを複製しません。

```nagi
class User:
    name: str
    score: i64

def main():
    users = [User(name="Nagi", score=10)]
    for user in users:
        print(user.name)
        print(user.score)
        name = copy(view(user.name))
        print(name)
    append(users, User(name="凪", score=20))
```

借用した要素は再代入できず、要素全体や非Copyフィールドを所有値として関数へ渡したり、返したりできません。文字列や配列を手元に残す場合は、上の例のようにフィールドを明示的にcopyします。ループ内で作ったフィールドのviewをループの外へ保存することもできません。非Copy enum・nullable・Resultのpayloadを`match`で取り出す借用は、まだ未対応です。

フィールドの借用は場所ごとに追跡します。例えば`view(data.values)`が生きていても、別の`data.name`を取り出せます。`data`全体や`data.values`の移動はできません。

viewが借りているデータの移動・再代入・appendも制限します。借用はコードのブロックをもとに追跡します。Rustほど細かく、借用を使い終わった位置を判定するわけではありません。

Nagiの検査は、Nagiのソース位置でmoveや借用の診断を返すためのものです。生成Rustの検査を代替するものではありません。複雑な分岐でのviewの再代入などでは、`check`が成功しても生成Rustの借用検査に失敗する場合があります。また、`check`がRust側なら有効なコードを保守的に拒否する場合もあります。

Rustアダプターとの型の一致、`copy`に必要な`Clone`、非同期処理の`Send`・共有状態の`Sync`も最終的には`build`で検査します。`shared[T]`で包むだけでTが並行処理に適した型になるわけではありません。実行ファイルを作るにはNagiとRustの両方の検査を通す必要があります。

元のデータとは別に保持したいときは、`copy(view(data))`で所有するコピーを作ります。
