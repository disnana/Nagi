# ローカルRustライブラリをNagiから使う

手書きのRust library crateを`path`依存で組み込むサンプルです。crateのpackage名は`nagi-pricing-engine`、アダプターから使う依存名は`pricing`です。`nagi.toml`で数量割引のfeatureを有効にし、既定の手数料featureを無効にします。

```text
rust-library/
  nagi.toml
  library-pricing.nagi
  native.rs
  engine/
    Cargo.toml
    src/lib.rs
```

リポジトリのルートから、依存tableに対応した`nagic`で実行します。PATHにない場合はコンパイラの絶対パスを使ってください。

```sh
nagic check --project test-nagi-code/rust-library
nagic run --project test-nagi-code/rust-library
```

プログラムは次を表示します。

```text
Workshop notebook
Subtotal cents:
3000
Discount cents:
300
Service fee cents:
0
Total cents:
2700
unit price must be nonnegative and quantity must be positive
Local Rust library verified.
```

単価250 centsの商品12個は、小計3000、割引300、手数料0、合計2700になります。コードはこれらをassertし、数量0を拒否する`Result`も検査します。計算用のcrateは標準ライブラリだけを使い、そのcrate自体の取得は不要です。Nagi runtimeの依存が未取得なら、初回ビルドではCargoが取得します。

## 設定と型の境界

```toml
[rust.dependencies]
pricing = { version = "0.1", path = "engine", package = "nagi-pricing-engine", features = ["volume-discount"], default-features = false }
```

`path`は`nagi.toml`基準なので、別の作業フォルダーから呼び出したり、`--out`で生成先を変えたりしても`engine/`を参照します。`version`も指定しているため、Cargoはローカルcrateのversionが条件に合うか検査します。

| 場所 | APIと役割 |
|---|---|
| [engine/src/lib.rs](engine/src/lib.rs) | `quote(&str, i64, i64) -> Result<Quote, QuoteError>`。独立したRustの料金計算 |
| [native.rs](native.rs) | `pricing::quote`を呼び、crateの`Quote`を生成されたNagiの`Quote`へ、エラーを`nagi_runtime::Error`へ変換 |
| [library-pricing.nagi](library-pricing.nagi) | `extern def`で型を宣言し、借用文字列を渡してclassとResultを受け取る |

Rust crateの`Quote`は商品名と小計・割引・手数料・合計を持ちます。料金は整数のcentsです。空白だけの商品名、負の単価、0以下の数量、整数範囲を超える計算は`QuoteError`になります。`volume-discount`は10個以上で小計の10%を割り引き、1 cent未満を切り捨てます。既定の`service-fee`は100 centsを加えますが、このアプリでは無効です。

`engine/Cargo.toml`の`[workspace]`でNagiリポジトリのworkspaceから独立させています。このcrateにはNagiへの依存がなく、別のRustアプリからも通常の`path`依存として使えます。Nagiのclassとの変換はアダプターに置くため、計算crateは生成コードの型を知る必要がありません。

`check`と`symbols`はRustの本体・featureによる動作を検証しません。その確認は`build`/`run`で行います。設定の仕様、featureの統合、生成したlockの扱いは[プロジェクト設定](../../docs/projects.md)を参照してください。crate単独の`Cargo.lock`や`target/`はサンプルに含めません。
