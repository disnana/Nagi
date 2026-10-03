# ファイルのimportとRust連携

別のNagiファイルは引用符付きの相対パスで読み込みます。`import "ファイル名" as 名前`でmodule名を付けるか、`from "ファイル名" import 定義名`で関数・classを選べます。Rustの関数を使う場合は、Nagiで引数・戻り値の型を宣言し、ビルド時にRustファイルを指定します。

以下のmodule名・fromの別名は、このリポジトリの最新ソースからビルドしたコンパイラで利用できます。

## Nagiファイルを分割する

同じディレクトリに次の2つのファイルを作ります。`models.nagi`にはclassを書きます。

```nagi
class Item:
    name: str
    count: i64
```

`app.nagi`で読み込みます。

```nagi
import "models.nagi"

def main():
    item = Item(name="Nagi", count=42)
    print(item.name)
    print(item.count)
```

```sh
nagic run app.nagi
```

`Nagi`と`42`が表示されます。パスはimportを書いたファイルのディレクトリを基準にします。Highは`.nagi`、Lowは`.low`を読み込めます。Lowではimportの末尾に`;`を置けます。

この従来のimportは、依存先も含めた定義を同じ名前空間に読み込みます。組み込み関数も従来どおり使えます。

### module名と定義の別名

`orders.nagi`に次の定義を置きます。

```nagi
class Order:
    amount: i64

def score(order: Order) -> i64:
    return order.amount
```

`app.nagi`ではmodule名とclassの別名を使えます。

```nagi
import "orders.nagi" as orders
from "orders.nagi" import Order as SavedOrder

def main():
    order: SavedOrder = orders.Order(amount=42)
    score = orders.score
    print(score(order))
```

`orders.Order`と`SavedOrder`は同じ型です。`orders.score(order)`で直接呼び出すこともできます。型引数・フィールド型・nullableでも`List[orders.Order]`や`orders.Order?`を使えます。別ファイルで定義した同名の`Order`は別の型になり、混ぜて渡すと型エラーになります。

module名で見えるのは、そのファイル自身が定義した関数とclassです。importした名前は自動で再公開しません。`from "orders.nagi" import Order`のように別名を省略することもできます。1つのfrom文で選ぶ定義は1つです。複数の定義は文を分けて読み込みます。`from`と`as`はimportの文脈だけで解釈し、関数や変数の名前にも使えます。関数内でmodule名と同じローカル名を使った場合は、現在のローカル変数の規則に従います。classのmethod呼び出しには対応していません。

同じ実ファイルは、複数のmodule名・fromの別名・従来のimportを使っても1回だけ読み込みます。循環するimport、見つからないファイル、HighとLowの混在はエラーです。存在しない定義のfrom importや、同じ場所で異なる定義を同じ名前にするimportは、そのimport文でエラーになります。`import sqlite`のように引用符なしで標準moduleを読み込む構文と、公開範囲の指定は未対応です。

### LowとRustで同じ定義を使う

Lowでも`import "orders.low" as orders;`と`from "orders.low" import Order as SavedOrder;`を使えます。Highから生成したLowにはmoduleと定義のIDを残すため、再parseや手書きLowとの統合でも型の区別を保ちます。

手書きLowでrootのmodule名`orders`にある関数を差し替える場合は、`@replace generated::orders::score`で対象を指定します。従来の`@replace generated::score`も使えます。引数・戻り値・asyncの一致などの条件は[Lowの差し替え](low-language.md)と同じです。

Rustのアダプターからは、rootで読み込んだmoduleのclassを`super::orders::Order`、fromの別名を`super::SavedOrder`で参照します。両方とも同じ生成型を指します。従来の平坦なimportの`super::Item`も使えます。内部の名前は衝突を避けて生成し、JSONのフィールド名やSQLの列名には元の名前を残します。[moduleの実行例](../test-nagi-code/library-examples/module-imports/README.md)では、同名classと関数の別名を試せます。

### 読み込みの上限とエラー位置

最大128ファイル、深さ64、合計8 MB、1ファイル2 MBです。型検査とLow統合のエラーは、元のファイル名と行番号を表示します。生成されたRustでのエラーも対応するNagi・Lowの行を先に表示します。対応する行が分からない場合や手書きRustのエラーは、Rustの位置を表示します。列位置の対応は未実装です。

### テキストファイルを埋め込む

`include_text("index.html")`は、ソースに隣接するUTF-8ファイルをビルド時に読み込み、`str`として埋め込みます。パスは文字列リテラルで指定してください。配布したアプリを実行するときには、元のファイルは不要です。

## Rustの関数を呼ぶ

次を`app.nagi`に保存します。`@rust`は呼び出すRustの関数を指定し、`extern def`はその引数・戻り値の型を宣言します。宣言に本体や末尾の`:`は付けません。

```nagi
@rust("native::text_bytes")
extern def text_bytes(text: view[str]) -> i64

def main():
    text = "Nagi"
    print(text_bytes(view(text)))
    print(text)
```

同じディレクトリの`native.rs`にRustの関数を書きます。

```rust
pub fn text_bytes(text: &str) -> i64 {
    text.len() as i64
}
```

```sh
nagic run app.nagi --rust native.rs
```

`4`と`Nagi`が表示されます。文字列は`view[str]`で借りているので、呼び出し後も使えます。`--rust`はRustファイルを`native`というモジュールとして組み込みます。関数は`pub`で公開し、宣言した型に合わせてください。たとえばNagiの`str`はRustの`String`、`view[str]`は`&str`です。

Lowでは`extern fn ...;`と書きます。Rustのasync関数には`extern async def`（Lowでは`extern async fn`）を使い、呼び出し側でawaitします。Rustの関数のパスは`native::`から始まる識別子の列です。extern関数は、viewを返す宣言やHTTP属性には対応していません。

### Rustのcrateを使う

Cargoの依存は`--rust-dep NAME=VERSION`で追加します。たとえばserde_jsonを使うアダプターでは、次のように指定します。

```sh
nagic run app.nagi --rust native.rs --rust-dep serde_json=1.0
```

初回はCargoが依存を取得するため、通常はネットワーク接続が必要です。ローカルcrateの`path`、`features`、依存名とpackage名を分ける`package`指定には、`nagi.toml`の依存tableを使います。[ローカルRustライブラリのサンプル](../test-nagi-code/rust-library/README.md)は、独立したcrateをアダプターから呼び、Rustの構造体・エラーをNagiのclass・Errorへ変換します。crateを使う完全な例は[リポジトリのRust連携サンプル](../test-nagi-code/rust-bridge/)にあります。入口・Rustファイル・依存を毎回指定せずに使う場合は、[nagi.tomlとプロジェクト](projects.md)に保存してください。CLIとVS Codeで同じ設定を使えます。

Nagiの`check`は、宣言した型と呼び出し、所有権、借用を検査します。`check`・`lower`・`symbols`はCargoを呼ばず、依存を取得しません。Rustの本体やcrateのAPIは検査しません。宣言とRustの実装が一致するかどうかは`build`で検査します。Rust固有の型を使う場合は、Rust側で数値・str・List・class・Resultなどへ変換してから渡してください。Rust側から生成したNagiのclassを参照する場合は、rootのimportに合わせて`super::型名`や`super::module名::型名`を使います。

生成したCargo.lockを保持して`cargo build --locked --manifest-path build/app/Cargo.toml`を実行すると、同じ依存の解決を再利用できます。通常の`nagic build`は生成したプロジェクトへの`cargo build --release`を実行し、既存のlockを保持します。`nagic build --locked`は未対応です。lockはローカルcrateのソース内容を固定しません。

この連携は同じRustビルド内で関数を呼び出します。安定したC ABIや、実行時にDLLを読み込む機能は未対応です。

共通コードを複数アプリで使う例、Rust側へNagiの関数を渡す例は[ライブラリとRustの資産](libraries.md)と[サンプル一覧](library-examples.md)にあります。現在の対応と、resource・ランタイム選択などの追加案は[ライブラリの設計](library-design.md)に整理しています。
