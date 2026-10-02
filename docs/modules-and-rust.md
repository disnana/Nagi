# ファイルのimportとRust連携

入口・Rustファイル・crate依存を保存する場合は[nagi.tomlとプロジェクト](projects.md)を使ってください。CLIとVS Codeで同じ設定を利用できます。

## Nagiファイルを分割する

```nagi
import "models.nagi"
import "validation.nagi"
```

パスはimportを書くソースファイルのディレクトリを基準にします。Highは`.nagi`、Lowは`.low`をimportできます。Lowでは文末に`;`を置けます。依存を先に読み、同じファイルは1回だけ読み込みます。循環import、見つからないファイル、言語の混在、名前の重複はエラーです。全ファイルは現在ひとつの名前空間に入り、名前付きmodule・alias・選択import・公開範囲はまだありません。

最大128ファイル、深さ64、合計8 MBです。各ファイルのparser上限は従来どおり2 MBです。型検査やLow統合の診断は元のimport先ファイルと行に戻します。Rust backendの診断も、対応する文・定義の行が分かる場合は元のNagi・Lowを先に表示します。元の位置が不明な診断や手書きRustは、Rustの表示を使います。厳密な列位置の対応は未実装です。

`include_text("index.html") -> str`はソースに隣接するUTF-8ファイルをコンパイル時に埋め込みます。パスは文字列リテラルで指定します。配布先でそのファイルを読み直す操作ではありません。

## Rustの関数を呼ぶ

Nagiに型付きの外部関数を宣言します。Highの`extern def`には本体や末尾の`:`を付けません。

```nagi
@rust("native::crc32")
extern def crc32(text: view[str]) -> i64

@rust("native::pretty_json")
extern def pretty_json(text: view[str]) -> Result[str, Error]
```

Lowでは`extern fn ...;`です。`extern async def` / `extern async fn`はRustのasync関数を呼びます。呼び出し側では通常のasync関数と同様にawaitします。Rustのパスは`native::`から始まる識別子列です。externの戻り値のviewや、externへのHTTP属性は現時点では受け付けません。

Rust側には通常の関数を書きます。

```rust
pub fn pretty_json(text: &str) -> Result<String, nagi_runtime::Error> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|e| nagi_runtime::Error::invalid(e.to_string()))?;
    serde_json::to_string_pretty(&value)
        .map_err(|e| nagi_runtime::Error::invalid(e.to_string()))
}
```

```powershell
.\target\release\nagic.exe run test-nagi-code/rust-bridge/bridge.nagi `
  --rust test-nagi-code/rust-bridge/native.rs --rust-dep serde_json=1.0
```

`--rust`は1つのRustファイルを`native` moduleとして組み込み、`--rust-dep NAME=VERSION`はCargo依存を追加します。追加依存はCargoによって解決され、通常は初回ビルドにネットワークが必要です。生成Cargo.lockを保持して`cargo build --locked --manifest-path build/bridge/Cargo.toml`で同じ解決を再利用できます。CLIは生成したcrateへの`cargo build --release`を呼びます。

Nagiは引数・戻り値・move/viewの宣言を検査し、Rust側の実装と型の一致はビルド時にrustcが検査します。`nagic check`だけではRustの本体やcrateのAPIは検査しません。Rust側では標準ライブラリ、crate、通常のRust moduleや`unsafe`実装を利用できます。Nagiの値型をRust側で使う場合は生成crateの`super::型名`を参照します。

これは同じRustビルド内での呼び出しで、安定したC ABIや実行時のDLL読み込みではありません。Rust固有の型を直接Nagiへ露出する機能や、Nagi自身で生pointer・unsafe構文を扱う機能は未実装です。まずRustのアダプターでprimitive・str・List・class・Resultへ変換してください。
