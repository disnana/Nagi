# HighとLow

Highは字下げでブロックを書く`.nagi`、Lowは波括弧と`;`で書く`.low`です。普段のアプリ開発にはHighを使います。Lowは生成された内容の保存・確認と、関数の実装の差し替えに使えます。波括弧で書きたい場合は、Lowだけでアプリを書いて実行することもできます。

型、所有権、借用、Resultの扱いはHighと共通です。Highから直接[Rustの関数を呼べる](modules-and-rust.md)ため、Rust連携に手書きLowは不要です。Lowへ移すだけで高速化したり、Rustの借用検査を回避したりはできません。Lowを独立した低水準言語として拡張することは、当面の開発対象にしていません。

通常のアプリを書くためにLowを先に学ぶ必要はありません。Python経験者は[言語ガイド](language-guide.md)からHighを書き始め、生成内容を調べたい場合や元のHighを保って関数を差し替えたい場合に、このページへ進めます。Lowは既存コードとの互換性も維持する別構文です。

## Lowだけで書いて実行する

コンパイラとビルド環境を[準備](getting-started.md)したら、作業用のフォルダーで次を`app.low`に保存します。

```low
fn total(price: i64, quantity: i64) -> i64 {
    return price * quantity;
}

fn main() -> unit {
    let amount = total(120, 3);
    print(amount);
}
```

```sh
nagic check app.low
nagic run app.low
```

`check`は型と所有権を検査し、`run`はビルドして実行します。実行結果は`360`です。Highファイルや`--native`の指定は必要ありません。

Lowでは`fn`で関数を宣言し、ブロックを`{ }`で囲み、文を`;`で区切ります。`amount`の型は呼び出し結果から推論されます。`let amount: i64 = total(120, 3);`のように型を明記することもできます。Highの`class`に相当する宣言は、Lowでは`record`です。

Lowを複数のファイルに分ける場合は、`import "orders.low" as orders;`のように相対パスで読み込みます。通常のファイルimportは、Highからは`.nagi`、Lowからは`.low`だけを読み込めます。HighとLowをつなぐ場合は、後述の`--native`による追加・差し替えを使います。

[注文の見積もりCLI](../test-nagi-code/low-examples/order-quote/README.md)では、Lowのファイル分割、record、Resultを使うアプリを試せます。

## HighをLowへ変換する

関数を定義して呼ぶ書き味はPythonに似ていますが、ここではNagiの型・所有権規則で検査します。生成内容を確認したい場合は、次の完全な例を`app.nagi`に保存します。

```nagi
def score(value: i64) -> i64:
    return value * 2

def main():
    print(score(7))
```

```sh
nagic run app.nagi
nagic lower app.nagi
```

実行結果は`14`です。`lower`は型・所有権を検査し、`build/app/generated.low`にLowを出力します。Lowでは関数を`fn`で宣言し、ブロックを`{ }`で囲みます。生成された変数には`let`と推論した型が付きます。

通常のHigh buildもHighを検査した後、Lowテキストへ変換して再解析・再検査します。LowはHighと共通の規則を持つ別構文で、安定したbackend非依存の意味論IRとして保証しているわけではありません。Lowを読み込むと、型と所有権を再検査します。保存Lowにはmoduleの識別情報を残しますが、元のHighへの診断位置の対応は保存しません。仕組みは[コンパイラの構成](compiler-internals.md)を参照してください。

`generated.low`を編集して変更を保存しようとするのがよくある間違いです。次のコマンドで再生成されるため、継続して差し替える実装は別の`native.low`へ書きます。

**一言でいうと：普段はHigh、生成の確認や関数の差し替えにはLow。** 型・所有権の規則は[型](types.md)と[所有権](ownership.md)、変換経路は[コンパイラの構成](compiler-internals.md)にあります。

## 手書きLowで関数を差し替える

次を同じディレクトリの`native.low`に保存します。

```low
@replace generated::score
fn optimized_score(value: i64) -> i64 {
    return value * 6;
}
```

```sh
nagic check app.nagi --native native.low
nagic run app.nagi --native native.low
```

今度は`42`と表示されます。High側の呼び出しは`score(7)`のままで、実行する本体がLowの`optimized_score`に置き換わります。`@replace generated::score`が差し替える対象を指定しています。

Highのrootで`import "orders.nagi" as orders`と読み込んだ関数には、`@replace generated::orders::score`で対象を指定します。fromの関数別名は`@replace generated::別名`で指定できます。型注釈の`orders.Order`やfromのclass別名も、同じ定義IDへ解決します。詳しくは[importとRust連携](modules-and-rust.md)を参照してください。

差し替え先は、引数の数・型、戻り値の型、asyncかどうかが元の関数と一致する必要があります。対象が存在しない場合や、同じ関数を複数回差し替える場合はエラーです。変更できるのは関数全体です。署名の一致は、元の関数と同じ動作になるという証明ではありません。この例は意図的に計算結果を変えています。

`generated.low`はコマンドを実行すると再生成されます。変更を残すには`native.low`へ書いてください。`--native`は差し替えに加えて通常のLow関数も追加でき、Highからその関数を呼べます。設定を保存する方法は[プロジェクト設定](projects.md#rustと手書きlowを追加する)にあります。

## Lowの文法

相対ファイルのimportには、`import "orders.low" as orders;`や`from "orders.low" import Order as SavedOrder;`を使えます。module名が公開するのは、そのファイル自身が定義した関数・record・enumです。

次はResultを処理する関数定義の断片です。単独で実行するには`main`から呼び出します。

```low
fn number_or(text: view[str], fallback: i64) -> i64 {
    match parse_i64(text) {
        case Ok(number) { return number; }
        case Err(_) { return fallback; }
    }
}
```

`Ok`と`Err`の両方が必要です。詳しくは[エラー処理](error-handling.md)を参照してください。

enumもHighと同じ種類とpayloadを持ち、すべての種類をmatchします。次も定義の断片です。

```low
enum Choice {
    Cancelled;
    Selected(id: i64);
}
fn selected_or_zero(choice: Choice) -> i64 {
    match choice {
        case Choice.Cancelled { return 0; }
        case Choice.Selected(id) { return id; }
    }
}
```

現在のLowは、型付きの変数、view、class（Lowではrecord）、enum、関数、分岐、ループ、async、scopeに対応しています。生ポインター、メモリ配置の指定、手動の確保・解放、SIMD命令、unsafe構文、C ABIは未対応です。Rustの関数を呼ぶ方法は[ファイルのimportとRust連携](modules-and-rust.md)にあります。
