# 2つのアプリで共有する料金計算の基盤

[対話CLI](../foundation-cli/README.md)と[JSONレポート](../foundation-report/README.md)が、同じNagiの窓口と同じRustの計算コードを使います。計算関数を引数として渡すため、呼び出し側の検証やデータ形式を保ったまま、Nagi実装とRust実装を選べます。

| ファイル | 役割 |
| --- | --- |
| `foundation.nagi` | `FoundationQuote`、入力検証、計算関数の選択、Nagiによる計算 |
| `pricing.rs` | Nagiの型や外部crateに依存しないRustの料金計算 |
| `bridge.rs` | Rustの`Totals`を、生成された`FoundationQuote`へ変換する境界 |

各アプリは`import "../shared/foundation.nagi"`を使い、`native.rs`から`#[path = "../shared/bridge.rs"] pub mod engine;`でRust側を取り込みます。2つの実行ファイルへ同じソースをそれぞれコンパイルする例です。

## 共有APIと契約

`foundation_select_engine(name: view[str])`は、`nagi`なら`foundation_nagi_quote`、`rust`なら`foundation_rust_quote`を返します。戻り値は`Result[fn[view[str], i64, i64, i64, Result[FoundationQuote, Error]], Error]`で、未対応の名前は入力エラーです。

`foundation_quote(calculate, label, unit_cents, quantity, discount_bps)`は下の条件を検証してから、渡された計算関数を呼びます。`label`は借用するので、元の文字列を後から使えます。成功した`FoundationQuote`はラベルの所有コピーと、入力値・小計・割引額・合計を持ちます。`Result`の失敗はCLIでは`try`で伝播し、レポートでは`match`で行ごとに処理します。

| 値 | 条件 |
| --- | --- |
| `label` | UTF-8で1〜80 byte。前後の空白を自動で除去しない |
| `unit_cents` | 0〜100,000,000の整数 |
| `quantity` | 1〜10,000の整数 |
| `discount_bps` | 0〜10,000。1000が10% |

小計は単価×数量、割引額は`subtotal × discount_bps / 10000`の整数除算で切り捨て、合計は小計−割引額です。この範囲なら乗算はi64に収まります。全項目が同じ通貨の最小単位である前提です。税・送料・通貨換算は含めません。

`foundation_nagi_quote`は上の範囲を検証済みの入力に使う計算関数です。通常は`foundation_quote`を通してください。Rustの`pricing::calculate`は独立したRustコードから呼べるように、数値の範囲も検証します。`bridge.rs`はラベルを検証し、Rust側の失敗を`nagi_runtime::Error::invalid`へ変換します。

## 自分のRust実装へ置き換える

同じ引数・戻り値の関数を`@rust`と`extern def`で宣言し、`foundation_quote`へ渡します。共有のNagiファイルを変更せず、各アプリの`native.rs`に自分の関数を公開できます。

```nagi
@rust("native::custom_quote")
extern def custom_quote(label: view[str], unit_cents: i64, quantity: i64, discount_bps: i64) -> Result[FoundationQuote, Error]
```

`main`内の呼び出しは、たとえば`quote = try foundation_quote(custom_quote, view(label), 999, 3, 1250)`です。Rust側の対応する型は`&str`、`i64`、`Result<crate::FoundationQuote, nagi_runtime::Error>`です。戻すclassは値を所有し、借用した入力の参照を保存しません。計算関数はアプリが信頼する実装で、窓口は独自実装が返した金額を再計算しません。

## 現在できる共有の範囲

この例はソースの再利用で、公開・配布されたNagiパッケージやCargo crateではありません。importした定義は同じ名前空間に入り、別名や可視性指定はありません。そのため公開する名前に`foundation_`／`Foundation`を付けています。

`rust.file`は1つのアダプターを指定し、複数のRustファイルはその下の通常のRust moduleとして組み込みます。既存crateのversion指定はできますが、`nagi.toml`の依存にCargoのpath・git・features指定はまだ書けません。Rust実装と宣言が合うかは`build`／`run`で確認します。安定したC ABIや実行時DLL読み込みの例でもありません。

アプリはDBを開かず、HTTPも起動しません。標準ランタイムのビルド依存は通常どおり含まれるため、ビルドにはRust/Cargoと対応するCビルド環境が必要です。共有ソースを変えたら、利用する両方のアプリを再ビルドしてください。

[English](README.en.md)
