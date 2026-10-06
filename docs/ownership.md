# 所有権

[目次](README.md) · [入門](language-guide.md#4-読むだけならviewで借りる) · [型](types.md)

関数に文字列を渡し、その関数が保持してよいようにするには、所有する値を渡します。Pythonではリストを別の名前へ代入すると、二つの名前が同じリストを指します。Nagiでは、値を手放す・読むために貸す・同じ値を共有する操作を区別します。

```nagi
def use_name(name: str):
    print(name)

def main():
    name = "alice"
    use_name(name)
    # print(name) はmove後の使用
```

この完全なコードは`alice`を表示します。呼び出した後、元の`name`ではその文字列を使えません。この引き渡しを**move**と呼び、後片付けの責任も新しい所有者へ移ります。moveそのものがcloseや破棄を行うわけではありません。

コメントを外して`print(name)`を追加すると、move後の使用として`check`が拒否します。読むだけなら、引数を`view[str]`にして`view(name)`を渡します。独立した文字列が必要なら`copy(view(name))`を渡します。[入門の実行例](language-guide.md#4-読むだけならviewで借りる)で試せます。

## 現在の代入と今後の変更

現在の`a = b`は、数値・boolなどCopyとして扱う値ならコピーします。文字列や配列など、非Copyの所有値ならmoveします。Pythonの参照代入とは動作が異なります。

```nagi
def main():
    count = 2
    same_count = count
    print(count + same_count)
    name = "Nagi"
    destination = name
    print(destination)
    name = "new"
    print(name)
```

出力は`4`、`Nagi`、`new`です。`destination = name`の後、古い文字列は`destination`から使います。元の`name`も新しい値を代入した後なら使えます。再代入より前に`name`を読むと`check`が拒否します。両方の文字列を残すなら`destination = copy(view(name))`にします。

現行のCopy規則にはview、同期関数値、UUID、timestampや、中身がCopy規則を満たすclass・enum・nullableも含まれます。これは現在のcheckerの規則であり、すべてのenumや小さいclassをコピーできるという新たな約束ではありません。[型](types.md)も参照してください。

今後の移行では、既存の非Copy所有値の`a = b`に明示的な操作を求める方針を採用しています。手放すならmove、読むならview、独立した値を作るならcopy、同じ値を複数箇所で持つならsharedを選びます。**この移行は未実装で、現在の暗黙moveは引き続き受理されます。** `a = User(...)`のように新しい値を作る式とは区別します。将来のCopy対象の正確な型表、構文、引数・return・フィールド取り出し・view・shared handleの扱いは詳細設計で定めます。[設計判断](../DESIGN.md)を参照してください。

## 同じ値を複数の場所で持つ

独立したコピーではなく、同じ値を複数箇所で保持したいときは`share`と`clone_shared`を使います。Pythonの二つの名前への参照代入では不要な、明示的な操作です。次は文字列フィールドを持つclassを使った完全な例です。

```nagi
class Label:
    text: str

def main():
    label = share(Label(text="Nagi"))
    another = clone_shared(label)
    duplicate = copy(view(another.text))
    print(label.text)
    print(duplicate)
```

出力は`Nagi`、`Nagi`です。`share(value)`は所有値を受け取り、`shared[T]`を返します。`clone_shared(label)`は同じ値を保持するhandleを増やし、中身全体はコピーしません。`copy(view(another.text))`は、それとは別に所有文字列を作ります。

`text = another.text`で共有値から非Copyフィールドを奪おうとすると、`check`が拒否します。読むか、上のようにコピーしてください。sharedの通常の参照から任意の書き換えはできません。内部に状態を持つ資源は、同期や操作条件を定めたAPIを使います。sharedに包んでも任意のTがthread-safeになるわけではなく、共有や独立copyができない資源もあります。[組み込み関数](builtins.md#共有と型のサイズ)と[並行処理](concurrency.md)を参照してください。

最後のshared handleが値を解放することと、外部サービスの停止が完了することは別です。共有参照が循環すると値を保持し続ける場合もあり、任意の共有グラフが自動回収される保証はありません。終了完了が必要なら、その資源のshutdown／close APIで確認します。

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

viewを含む戻り値でも、`return None`や`return []`、`return ok(None)`のように借用を含まない値を直接返せます。viewを含む型のローカル変数を返す場合は、現在の値が空でも借用元を追跡できる必要があります。

`return`で終わるブロックの直下では、viewを一時的にローカル値へ切り替え、その後で引数のviewに戻して返せます。生成Rustでは、各代入を別の借用として扱います。分岐後や次のループで使う値の更新は維持し、所有値のコピーは追加しません。[実行テスト](../compiler/tests/view_branch_rebinding.rs)でHigh・保存Low・手書きLowを確認しています。修正の収録状況は[CHANGELOG](../CHANGELOG.md)を参照してください。

借用を含むList・Result・Optionを一時的にローカル値へ切り替え、引数の借用へ戻して返せます。別の変数へのmove、入れ子の値、`if`／`match`の分岐、`for`／`while`、asyncも対象です。生成時に代入前後の格納先を分け、右辺の評価後に古い値を解放します。所有値のコピーで寿命を延ばす処理は追加しません。[生成と実行のテスト](../compiler/tests/view_flow_completion.rs)と[確保・破棄のテスト](../compiler/tests/view_container_drop.rs)でHigh・保存Low・手書きLowを確認しています。収録版は[CHANGELOG](../CHANGELOG.md)に記載します。

この処理をscope内で行う場合も、正常終了、エラー伝播、親タスクの取消し、本体のpanicを[実ランタイムのテスト](../compiler/tests/scope_runtime_contract.rs)で確認しています。通常のエラー出口では子の取消完了を待ちます。親Futureの破棄やpanicでは停止を要求しますが、同期的な破棄だけで子の終了完了までは待てません。

借用を復元すれば任意のコードが通るわけではありません。checkerは、局所的な借用が外へ漏れないことを引き続き検査します。たとえば借用を含むローカル変数の返却では、値が現在空でも追跡できる借用元が必要です。利用者が定義する借用を含むclass・enumや、任意のasync関数値は未対応です。

Nagiの検査は、Nagiのソース位置でmoveや借用の診断を返すためのものです。生成Rustの検査を代替するものではありません。また、`check`がRust側なら有効なコードを保守的に拒否する場合もあります。

Rustアダプターとの型の一致、`copy`に必要な`Clone`、非同期処理の`Send`・共有状態の`Sync`も最終的には`build`で検査します。`shared[T]`で包むだけでTが並行処理に適した型になるわけではありません。実行ファイルを作るにはNagiとRustの両方の検査を通す必要があります。

元のデータとは別に保持したいときは、`copy(view(data))`で所有するコピーを作ります。
