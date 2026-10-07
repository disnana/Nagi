# ファイルのimportとRust連携

別のNagiファイルは引用符付きの相対パスで読み込みます。`import "ファイル名" as 名前`でmodule名を付けるか、`from "ファイル名" import 定義名`で関数・class・enumを選べます。Rustの関数を使う場合は、Nagiで引数・戻り値の型を宣言し、ビルド時にRustファイルを指定します。

以下のmodule名・fromの別名は、Nagi 0.1.8から利用できます。

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

module名で見えるのは、そのファイル自身が定義した関数・class・enumです。importした名前は自動で再公開しません。`from "orders.nagi" import Order`のように別名を省略することもできます。複数の定義は`from "orders.nagi" import Order as SavedOrder, score`のように`,`で選べます。末尾の余分な`,`は付けません。`from`と`as`はimportの文脈だけで解釈し、関数や変数の名前にも使えます。関数内でmodule名と同じローカル名を使った場合は、現在のローカル変数の規則に従います。classのmethod呼び出しには対応していません。

同じ実ファイルは、複数のmodule名・fromの別名・従来のimportを使っても1回だけ読み込みます。循環するimport、見つからないファイル、HighとLowの混在はエラーです。存在しない定義のfrom importや、同じ場所で異なる定義を同じ名前にするimportは、そのimport文でエラーになります。引用符なしのimportは登録済みの`std.http.server`、`std.actor`、`std.result`等を対象にします。Nagi 0.1.11以降では`std.auth`、`std.ownership`、`std.task`も利用できます。利用するcompilerの版を確認してください。一般packageの探索と公開範囲の指定は未対応です。

`std.auth`のexperimental APIはNagi 0.1.11への導入対象で、認証済みの`Principal`と消費型の`Grant[P]`を扱います。公開配布での利用可否はRelease記録で確認してください。Rustの検証処理とNagiの独自policyをつなぐ例は[認証・認可サンプル](../test-nagi-code/application-examples/auth-boundary/README.md)にあります。固定credentialの実験で、JWS検証や全routeの認可チェックを提供するものではありません。

### LowとRustで同じ定義を使う

Lowでも`import "orders.low" as orders;`と`from "orders.low" import Order as SavedOrder;`を使えます。Highから生成したLowにはmoduleと定義のIDを残すため、再parseや手書きLowとの統合でも型の区別を保ちます。

手書きLowでrootのmodule名`orders`にある関数を差し替える場合は、`@replace generated::orders::score`で対象を指定します。従来の`@replace generated::score`も使えます。引数・戻り値・asyncの一致などの条件は[Lowの差し替え](low-language.md)と同じです。

Rustのアダプターからは、rootで読み込んだmoduleのclassを`super::orders::Order`、fromの別名を`super::SavedOrder`で参照します。両方とも同じ生成型を指します。従来の平坦なimportの`super::Item`も使えます。内部の名前は衝突を避けて生成し、JSONのフィールド名やSQLの列名には元の名前を残します。[moduleの実行例](../test-nagi-code/library-examples/module-imports/README.md)では、同名classと関数の別名を試せます。

### 読み込みの上限とエラー位置

最大128ファイル、深さ64、合計8 MB、1ファイル2 MBです。型検査とLow統合のエラーは、元のファイル名と行番号を表示します。生成されたRustでのエラーも対応するNagi・Lowの行を先に表示します。対応する行が分からない場合や手書きRustのエラーは、Rustの位置を表示します。列位置の対応は未実装です。

### テキストファイルを埋め込む

`include_text("index.html")`は、ソースに隣接するUTF-8ファイルをビルド時に読み込み、`str`として埋め込みます。パスは文字列リテラルで指定してください。配布したアプリを実行するときには、元のファイルは不要です。

## 標準HTTP libraryを読み込む

`std.http.server`と`std.actor`はNagi 0.1.8から使える標準ライブラリです。

```nagi
import std.http.server as http
from std.http.server import Request, Response, Status as Code

def main():
    response = http.text(Code.OK, "Hello, Nagi!")
    print(response.status.value)
```

標準moduleはコンパイラと一緒に提供する登録済みのlibraryです。カレントディレクトリの同名ファイルやエディターのbufferで差し替えません。module importには`as`を付け、`from`では必要な定義だけを選べます。importするだけでlistenerやworkerは起動しません。生成Lowにも標準定義のIDを残し、標準型のaliasは同じnative型を指します。

`Request`などのresource型は[型の資料](types.md#標準httpのresource型)を参照してください。`App[State, E]`はAppごとにstateとエラー変換を持ち、Dbなしでも使えます。`route`は名前付きasync handlerまたはそのローカルaliasを受け取り、`route_mapped`はroute専用の変換を指定します。一般のasync関数値を引数やclassへ保存する制約は変わりません。型と所有権は`check`で、handler futureの`Send + 'static`と共有Stateの`Send + Sync`はRust buildで検査します。起動・応答・headersの操作は[HTTP](http.md)にあります。

## 標準actor libraryを読み込む

actorも同じimportの規則を使います。

```nagi
import std.actor as actor
from std.actor import Actor as Worker, CallError
```

`Worker[M, R, E]`と`actor.Actor[M, R, E]`は同じnative型です。`Supervisor[C]`へ名前付きasyncのfactoryとhandlerを登録し、`Turn[S, R, E]`で次の状態と返信を返します。メッセージ・返信・業務エラーは容量を数えられる所有値が必要で、Map・view・shared・opaque resourceを含められません。[actor](actor.md)・[APIリファレンス](actor-reference.md)・[サンプル](../test-nagi-code/library-examples/supervised-service/README.md)に実際の署名と手順があります。

## 標準の値転送操作を読み込む

`std.ownership.move`はNagi 0.1.11で公開済みです。利用するcompilerが0.1.11以降であることを確認してください。module名と定義の別名のどちらでも呼び出せます。次の完全な例は`Nagi`を表示します。

```nagi
import std.ownership as ownership
from std.ownership import move as transfer

def main():
    first = "Nagi"
    second = ownership.move(first)
    third = transfer(second)
    print(third)
```

所有する非Copyローカルそのものの代入には明示操作を使います。新値生成やCopy代入、引数・return・field/index等の既存consume規則は維持します。`move`は予約語ではなく、同名のユーザー関数は通常の関数です。標準操作自体を第一級関数値にすることはできません。引数は一つ、型は入力から推論し、明示型引数は受け付けません。借用やFutureの制約と正確な移行範囲は[所有権](ownership.md#代入と明示move)を参照してください。

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

[Axumの見積API](../test-nagi-code/application-examples/axum-service/README.md)は、HTTPのルーティングとJSONの読み取りをRustに、型付きの検証と計算をNagiに置く例です。Rustアダプターから名前を指定してNagiのasync関数をawaitし、`Result`を受け取ります。そのNagi関数からもRustのasync処理をawaitします。これは生成された特定の関数を直接呼ぶ方法で、任意のasync関数値をextern引数で渡す機能ではありません。

### Rustのcrateを使う

Cargoの依存は`--rust-dep NAME=VERSION`で追加します。たとえばserde_jsonを使うアダプターでは、次のように指定します。

```sh
nagic run app.nagi --rust native.rs --rust-dep serde_json=1.0
```

初回はCargoが依存を取得するため、通常はネットワーク接続が必要です。ローカルcrateの`path`、`features`、依存名とpackage名を分ける`package`指定には、`nagi.toml`の依存tableを使います。[ローカルRustライブラリのサンプル](../test-nagi-code/rust-library/README.md)は、独立したcrateをアダプターから呼び、Rustの構造体・エラーをNagiのclass・Errorへ変換します。生成されたNagiの名前への参照はアダプターにまとめ、独立したcrateにはそのcrate自身の型とAPIを残します。crateを使う完全な例は[リポジトリのRust連携サンプル](../test-nagi-code/rust-bridge/)にあります。入口・Rustファイル・依存を毎回指定せずに使う場合は、[nagi.tomlとプロジェクト](projects.md)に保存してください。CLIとVS Codeで同じ設定を使えます。

Nagiの`check`は、宣言した型と呼び出し、所有権、借用を検査します。`check`・`lower`・`symbols`はCargoを呼ばず、依存を取得しません。Rustの本体やcrateのAPIは検査しません。宣言とRustの実装が一致するかどうかは`build`で検査します。登録済み標準resourceは対応するnative型を使えます。それ以外のRust固有の型は、Rust側で数値・str・List・class・Resultなどへ変換してから渡してください。Rust側から生成したNagiのclassを参照する場合は、rootのimportに合わせて`super::型名`や`super::module名::型名`を使います。

生成したCargo.lockを保持して`cargo build --locked --manifest-path build/app/Cargo.toml`を実行すると、同じ依存の解決を再利用できます。通常の`nagic build`は生成したプロジェクトへの`cargo build --release`を実行し、既存のlockを保持します。`nagic build --locked`は未対応です。lockはローカルcrateのソース内容を固定しません。

この連携は同じRustビルド内で関数を呼び出します。安定したC ABIや、実行時にDLLを読み込む機能は未対応です。

Rustの型不一致は、対応するNagi宣言や文のファイル・行に表示します。生成Rustの詳細が必要な場合は`nagic build app.nagi --rust-diagnostics`を使います。手書きRustや依存crateの診断はRust位置のまま残ります。unsafe、panic、trait、暗号やpolicyの正しさはexternの型宣言だけでは検査できません。

共通コードを複数アプリで使う例、Rust側へNagiの関数を渡す例は[ライブラリとRustの資産](libraries.md)と[サンプル一覧](library-examples.md)にあります。現在の対応と、resource・ランタイム選択などの追加案は[ライブラリの設計](library-design.md)に整理しています。
