# Cloudへ開発を引き継ぐ

2026-10-01時点の引き継ぎです。実装の基準は`main`の`49ab41f`です。この文書を追加したコミット以降から作業してください。開始時に現在のHEAD・差分・CIを確認し、この記録から変わった点を把握してください。

## 完了した実装

- `nagi.toml`で入口・Rust依存・手書きLowを設定。CLIとVS Codeで共通に使う。
- High / LowのResult専用`match`。`case Ok(value)`と`case Err(problem)`が各1回必要。型・move・借用・全経路returnを検査する。
- `not_found`、`internal_error`、`fail`、`error_kind`、`error_message`。HTTPの400・404・500や代替データへの回復を試す[Result API](../test-nagi-code/result-api/README.md)を用意。
- `nagic symbols`で元ソースの定義・参照位置と、関数・classの宣言の型をJSON出力。型検査やビルドを行わず、型エラーがあっても構文・importが読めれば取得できる。
- `symbols --editor-input`で標準入力のNagi/Lowバッファをメモリ上で読み込む。元ファイルは書き換えない。既存ファイルに対応し、設定ファイルは保存済みのものを使う。
- VS Code拡張0.1.3：F12、宣言の型ホバー、関数・class・型・代表的な組み込み関数の補完、引数ヒント。未保存のimport先も読む。書きかけで解析できない場合は保存済みの宣言を使い、表示上も区別する。

書き方は[docsの目次](README.md)、IDE機能は[拡張README](../editors/vscode-nagi/README.md)を参照。

## 次の作業候補

前の会話で提案した次の段階は、**ローカル変数の推論型表示と`value.field`の補完**。まだ実装していない。Cloudでこの作業を依頼された場合は、次を進める。

1. checkerの型情報とスコープを追い、引数・型注釈付き変数・推論したローカル変数の型をIDE向けに取得する。型エラーがある場合の宣言情報取得を維持する。
2. ホバーで実際の変数の型を表示し、classの値の`.`ではそのclassのフィールドだけを候補にする。nullable・Result・配列を勝手にunwrapして候補を作らない。
3. if・loop・matchのcaseのスコープ、同名変数、import、High/Low、UTF-16列位置、未保存バッファ、書きかけの構文を検証する。解析できない箇所に型を推測して表示しない。
4. 型ホバー・補完・F12・診断・ビルド・実行の既存動作を維持し、対応範囲をdocsに反映する。拡張の機能を増やしたらversionも更新する。

現在のsymbolsは宣言情報を対象にする。ローカル変数・caseのpayload・値のフィールドの推論表示、rename、デバッグは未対応。名前付きmodule、汎用generic、nullableのmatch、生pointer/unsafe/C ABIも今後の範囲で、このIDE作業に必須ではない。

## 主に読むコード

| ファイル | 内容 |
|---|---|
| [compiler/src/ast.rs](../compiler/src/ast.rs) | `Expr.ty`、代入の型注釈、関数・class・matchのAST |
| [compiler/src/check.rs](../compiler/src/check.rs) | 型検査、変数のスコープ、move・borrow、式の型付け |
| [compiler/src/source.rs](../compiler/src/source.rs) | import、元ソースへの位置対応、未保存バッファ |
| [compiler/src/symbols.rs](../compiler/src/symbols.rs) | 宣言・参照・型情報のJSON出力 |
| [compiler/src/emit.rs](../compiler/src/emit.rs) | CLIの読み込み・symbols・検査・生成の順序 |
| [editors/vscode-nagi/src/extension.js](../editors/vscode-nagi/src/extension.js) | VS Code provider、未保存対応、キャンセル・古い結果の破棄 |
| [editors/vscode-nagi/src/features.js](../editors/vscode-nagi/src/features.js) | ホバー対象、補完の挿入、型の文脈、引数位置 |
| [compiler/tests/symbols.rs](../compiler/tests/symbols.rs) | 実CLI、位置、型、バッファ、ファイルを変更しないことの検証 |
| [editors/vscode-nagi/test/features.test.js](../editors/vscode-nagi/test/features.test.js) | 補完・ホバー・引数位置の単体テスト |
| [editors/vscode-nagi/test/host.js](../editors/vscode-nagi/test/host.js) | 実VS CodeのF12・補完・ホバー・引数ヒント・実行テスト |

## 確認済みの結果

- ローカルWindows：Rust 93テスト、Node 15テスト、fmt・clippy、releaseコンパイラのビルドが通過。
- 配布用VSIX 0.1.3の中身を読み込む実VS Codeで、ホバー・補完・名前付き引数・未保存のimport・書きかけでの回復・引数位置・High/Low・F12・プロジェクト実行を確認。
- Result APIは起動したネイティブサーバーに15件のHTTPリクエストを送り、ステータス・Content-Type・JSONを照合済み。
- `49ab41f`の[Linux CI](https://github.com/disnana/Nagi/actions/runs/36733479074)は成功。fmt、clippy、Rust/Nodeテスト、ビルド、既存サンプル、実HTTP統合、fuzz-smokeが通過。

これらは引き継ぎ時点の結果。Cloudで変更した後の検証とは分けて報告する。実VS CodeのGUIテストはLinux CIには含まれていない。

## Cloudで検証する

リポジトリは`disnana/Nagi`。Cloudの環境にはRust stable（rustfmt・clippy）、Node.js 22、Python 3.12、Cコンパイラを用意する。Cargo依存を取得してから検査・ビルドする。設定やネットワーク制約は、そのCloud環境の実際の状態を確認する。

リポジトリのルートで、まず次を実行する。

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
cargo build --release --examples --bins --locked
node --test editors/vscode-nagi/test/*.test.js
```

追加の既存サンプル・HTTP・fuzz検査は[CI](../.github/workflows/ci.yml)に従う。`scripts/build_examples.py`は追跡中の`benchmarks/results/examples.json`を更新するため、前後の差分を確認し、試験で生じたレポートを無関係な変更としてコミットしない。既存の変更がある場合は上書きしない。

Result APIを試す場合は`./target/release/nagic run --project test-nagi-code/result-api`で起動する。別のターミナルから`python3 test-nagi-code/result-api/smoke_api.py --base-url http://127.0.0.1:8097`を実行する。smokeスクリプトはHTTP送信と応答照合だけに保つ。Python側でサーバーを起動・停止したり、DBやファイルを作ったりしない。

VSIXは`python3 editors/vscode-nagi/scripts/package_vsix.py`で再作成する。GUIのあるVS Code環境が使える場合はHostテストも行い、使えない場合は未検証と報告する。

`build/`・`target/`・`native-target/`はGitの対象外。ローカルのexe・VSIX・テストログ・起動中のサービスはCloudのcheckoutに入らない。ソースから再作成し、既存のローカルサービスをCloudの`127.0.0.1`から呼べると仮定しない。実行環境の違いは[公式説明](https://learn.chatgpt.com/docs/environments/modes)も参照。

## 作業と報告

関連する変更だけをステージし、確認しながら意味のある単位でコミットする。最後にまとめてプッシュする。CloudでPRを使う場合は、新しい`codex/`ブランチからPRを作り、検証結果と未検証の範囲を記載する。

ユーザーへの説明とdocsは日本語で、何が動くか・どう試すかを先に示す。ローカル・Cloud・CI・実VS Codeの結果を区別し、実装した機能と次の作業候補を分けて報告する。
