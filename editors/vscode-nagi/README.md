# Nagi Language for VS Code

Nagi High (`.nagi`) とLow (`.low`)の開発補助です。

- 構文の色付け、コメント、括弧・引用符、4空白のインデント
- class・関数・HTTP・借用・Low置換のスニペット
- ファイルを開いた時と保存時の`nagic check`、Problemsへの診断表示
- コマンドパレットの型検査、Low変換、ビルド、実行
- エディター右上の実行ボタン
- `nagi.toml`の入口・Rust依存・Low設定を使ったプロジェクトの検査と実行
- F12 /「定義へ移動」で関数・class・import先へ移動

## インストール

リポジトリのルートで `python editors/vscode-nagi/scripts/package_vsix.py` を実行すると、`build/distribution/nagi-language-0.1.2.vsix` ができます。VS Codeの「拡張機能: VSIXからのインストール」で選択するか、次のコマンドを実行してください。

```powershell
code --install-extension build/distribution/nagi-language-0.1.2.vsix
```

色付けとスニペットはコンパイラなしで利用できます。型検査や実行には`nagic`が必要です。Nagiリポジトリを開き、ルートで `cargo build --release --locked -p nagic` を実行してください。リポジトリのrelease・debugビルド、次にPATHからコンパイラを探します。

## 設定

```json
{
  "nagi.compilerPath": "target/release/nagic.exe",
  "nagi.checkOnSave": true,
  "nagi.nativeFiles": [],
  "nagi.rustFile": "",
  "nagi.rustDependencies": [],
  "nagi.checkTimeoutMs": 15000
}
```

Windows以外の実行ファイルは`nagic`です。`compilerPath`・`nativeFiles`・`rustFile`の相対パスはワークスペースのフォルダーを基準にします。型検査の生成Lowは、プロジェクトまたはソース別の`build/vscode-nagi/`に出力し、通常のビルド出力と分離します。

未保存の編集中コードは自動検査しません。編集中は古い診断を消し、保存後に再検査します。手動コマンドは編集中のファイルを保存してから実行します。未信頼のワークスペースではコンパイラを実行しません。診断位置は現在のコンパイラに合わせて行単位です。

## プロジェクト

入口・Rust依存・手書きLowの設定は[nagi.toml](../../docs/projects.md)にまとめられます。開いているファイルから親へ最も近い`nagi.toml`を選び、コンパイラに`--project`として渡します。補助ファイルを開いていても、`entry`から検査・実行します。プロジェクト内の編集中ファイルがある間は自動検査を待ち、手動コマンドではそのプロジェクト内のファイルを保存します。設定の保存・作成・削除でも検査を更新します。

Rust連携サンプルは`test-nagi-code/rust-bridge/bridge.nagi`を開くだけで設定を使えます。`nagi.rustFile`などの従来の設定はコマンド引数として追加し、設定ファイルに対して上書き・追加するルールはCLIと共通です。`nagic check`はRust側の実装を検査せず、実装との型の一致はビルドで検査します。import先の型エラーはそのファイルのProblemsに表示します。

設定ファイルがなければ、従来どおり開いたファイルを単独で処理します。プロジェクト機能には、このリポジトリの最新版の`nagic`が必要です。

## 定義へ移動（0.1.2）

ファイルを保存してから、関数の呼び出し・classの型注釈や生成箇所にカーソルを置いてF12を押します。`import "models.nagi"`の文字列では、そのファイルの先頭へ移動します。High・Low両方に対応し、プロジェクトの入口から読み込まれたファイルや手書きLowの定義を対象にします。

たとえば`test-nagi-code/result-api/server.nagi`の`read_item`から`storage.nagi`へ、`Item`から`models.nagi`へ移動できます。ソースの場所はコンパイラの`symbols`コマンドから取得します。型エラーがあるコードでも構文とimportが読み込めれば使えます。

プロジェクト内に未保存のファイルがある間は移動せず、保存するよう案内します。単独ファイルも保存が必要です。読み込みが失敗した場合はOutputの「Nagi」で確認できます。

対象は関数・class・importです。ローカル変数・引数・caseのpayload、組み込み関数、Rust実装への移動は未対応です。Lowの置換関数とHighの宣言が両方ある場合は、Highの宣言を優先します。

## 開発

Node.jsで `node --test editors/vscode-nagi/test/*.test.js` を実行できます。VS CodeのExtension Development Hostでは、`--extensionDevelopmentPath`にこのフォルダーを指定して試せます。LSPによる型に基づく補完、rename、デバッグはこの版の範囲に含みません。
