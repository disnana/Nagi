# ファイルのimportとRust連携

別のNagiファイルは`import "ファイル名"`で読み込みます。Rustの関数を使う場合は、Nagiで引数・戻り値の型を宣言し、ビルド時にRustファイルを指定します。

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

同じファイルは1回だけ読み込みます。循環するimport、見つからないファイル、HighとLowの混在、名前の重複はエラーです。読み込んだ定義は同じ名前空間に入ります。`import sqlite`のような名前付きモジュールや、`as`・`from`・公開範囲の指定は未対応です。

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

初回はCargoが依存を取得するため、通常はネットワーク接続が必要です。crateを使う完全な例は[リポジトリのRust連携サンプル](../test-nagi-code/rust-bridge/)にあります。入口・Rustファイル・依存を毎回指定せずに使う場合は、[nagi.tomlとプロジェクト](projects.md)に保存してください。CLIとVS Codeで同じ設定を使えます。

Nagiの`check`は、宣言した型と呼び出し、所有権、借用を検査します。Rustの本体やcrateのAPIは検査しません。宣言とRustの実装が一致するかどうかは`build`で検査します。Rust固有の型を使う場合は、Rust側で数値・str・List・class・Resultなどへ変換してから渡してください。Rust側から生成したNagiのclassを参照する場合は、`super::型名`を使います。

生成したCargo.lockを保持して`cargo build --locked --manifest-path build/app/Cargo.toml`を実行すると、同じ依存の解決を再利用できます。通常の`nagic build`は生成したプロジェクトへの`cargo build --release`を実行します。

この連携は同じRustビルド内で関数を呼び出します。安定したC ABIや、実行時にDLLを読み込む機能は未対応です。
