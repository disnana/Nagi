# 共有基盤を使う対話型見積CLI

ラベル・単価・数量・割引率を入力すると、1件の見積をJSONで表示します。[共通のNagi窓口とRust module](../shared/README.md)を、[一括レポート](../foundation-report/README.md)と共有します。DBやサーバーを用意する必要はありません。

リポジトリのルートで、インストール済みの`nagic`を使います。

```sh
nagic check --project test-nagi-code/library-examples/foundation-cli
nagic run --project test-nagi-code/library-examples/foundation-cli
```

表示された順に`Notebook`、`999`、`3`、`1250`を入力します。単価はcentなどの最小通貨単位、割引率はbasis pointで、1250は12.5%です。結果の小計は2997、割引額は374、合計は2623になります。ラベルを借用して呼び出した後も、元の`Notebook`を表示できます。

既定ではNagiの計算関数を使います。Rustの共有計算へ替える場合は、bashで次のように実行します。

```sh
printf 'Notebook\n999\n3\n1250\n' | NAGI_PRICING_ENGINE=rust nagic run --project test-nagi-code/library-examples/foundation-cli
```

PowerShellの場合:

```powershell
$env:NAGI_PRICING_ENGINE = "rust"
@("Notebook", "999", "3", "1250") | nagic run --project test-nagi-code/library-examples/foundation-cli
```

`NAGI_PRICING_ENGINE=nagi`と`rust`は同じ金額・丸め規則を使います。それ以外の名前はエラーです。整数として読めない入力や数量0などは`Result`の失敗を`try`で伝播し、非ゼロで終了します。入力条件と自分のRust関数を差し込む方法は[共有APIの説明](../shared/README.md)を参照してください。

`nagi.toml`は`foundation_cli.nagi`と薄い`native.rs`を指定します。Rust moduleは`../shared/bridge.rs`から組み込み、既存の`@rust("native::engine::foundation_rust_quote")`を呼びます。依存crateの追加はありません。両方の実装を同じアプリに含め、選ぶ関数を切り替えるサンプルです。

生成ソースはこのフォルダーの`build/foundation_cli/`、通常の実行ファイルは`build/native-target/release/nagi-foundation-cli`です。Windowsでは`.exe`が付き、`NAGI_NATIVE_TARGET_DIR`を設定した場合は出力先が変わります。ビルドにはRust/CargoとCビルド環境が必要です。

[English](README.en.md)
