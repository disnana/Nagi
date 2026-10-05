# 共有料金計算を使うJSONレポート

複数行の料金を計算し、成功した見積・拒否した行・合計を1つのJSONにします。[対話CLI](../foundation-cli/README.md)と同じ[Nagi moduleとRust関数](../shared/README.md)を使います。CLIが1件の失敗で終了するのに対し、このアプリは`match`で各行の失敗を回収し、次の行を処理します。

リポジトリのルートで実行します。

```sh
nagic check --project test-nagi-code/library-examples/foundation-report
nagic run --project test-nagi-code/library-examples/foundation-report
```

既定ではRustの計算関数を使います。入力は[report-input.json](report-input.json)で、`include_text`によりビルド時にexeへ埋め込みます。ファイルを編集した後は再ビルドしてください。配布したexeは元のJSONファイルを読みません。

| 入力ID | 結果 |
| --- | --- |
| 1 | 単価999×数量3、12.5%割引。合計2623 |
| 2 | 単価250×数量2、割引なし。合計500 |
| 3 | 数量0のため拒否。`quantity must be between 1 and 10000` |

出力の`quotes`にはID1とID2の`quote`、`rejected`にはID3と`reason`、`total_cents`には3123が入ります。行の料金エラーはレポートに含めるため、上の入力でプログラムは正常終了します。入力JSONの構造が不正、余分なfieldがある、1000行を超える、エンジン名が不正、といったレポート全体の失敗は非ゼロで終了します。IDは入力の識別子をそのまま返し、一意性は検証しません。

Nagiの計算関数でも同じレポートになります。

```sh
NAGI_PRICING_ENGINE=nagi nagic run --project test-nagi-code/library-examples/foundation-report
```

PowerShellの場合:

```powershell
$env:NAGI_PRICING_ENGINE = "nagi"
nagic run --project test-nagi-code/library-examples/foundation-report
```

入力classは数値fieldだけなのでCopyとして反復できます。出力のclassはラベルなどを所有し、型を付けたListへ`append`します。1000行と共有APIの価格上限により、合計もi64の範囲に収まります。料金の単位・丸め・検証条件とRustへの置き換えは[共有APIの説明](../shared/README.md)を参照してください。

`nagi.toml`は`foundation_report.nagi`と`native.rs`を指定します。Rust側はCLIと同じ`../shared/bridge.rs`と`pricing.rs`です。DB・HTTP・追加crateを使いません。生成ソースはこのフォルダーの`build/foundation_report/`です。実行ファイルは`native:`行のパスで確認します。次の未リリース版では既定名にも識別子が付き、`build/native-target/release/nagi-foundation-report-<識別子>`となります。Windowsでは`.exe`が付き、`NAGI_NATIVE_TARGET_DIR`で出力先を変更できます。ビルドにはRust/CargoとCビルド環境が必要です。

[English](README.en.md)
